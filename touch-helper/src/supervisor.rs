//! The supervisor owns no evdev/uinput descriptors. A stuck worker is killed locally.
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Exited(i32),
    HeartbeatExpired,
    HardDeadline,
    OutputFailed,
}

struct Worker(Child);
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub fn supervise(
    mut command: Command,
    hard_limit: Duration,
    heartbeat_limit: Duration,
) -> io::Result<Outcome> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());
    let mut worker = Worker(command.spawn()?);
    let stdout = worker.0.stdout.take().expect("piped worker stdout");
    let start = Instant::now();
    let heartbeat = Arc::new(AtomicU64::new(0));
    let received = heartbeat.clone();
    let output_state = Arc::new(AtomicU8::new(0)); // 0: reading, 1: drained, 2: failed
    let completed = output_state.clone();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let Ok(count) = reader.by_ref().take(16385).read_line(&mut line) else {
                completed.store(2, Ordering::Release);
                break;
            };
            if count == 0 {
                completed.store(1, Ordering::Release);
                break;
            }
            if count > 16384 || !line.ends_with('\n') {
                completed.store(2, Ordering::Release);
                break;
            }
            if line.trim() == "{\"event\":\"heartbeat\"}" {
                received.store(start.elapsed().as_millis() as u64, Ordering::Release);
            } else if io::stdout().write_all(line.as_bytes()).is_err() {
                completed.store(2, Ordering::Release);
                break;
            }
        }
    });
    let mut exit_status = None;
    loop {
        if exit_status.is_none() {
            exit_status = worker.0.try_wait()?;
        }
        match output_state.load(Ordering::Acquire) {
            1 if exit_status.is_some() => {
                return Ok(Outcome::Exited(exit_status.unwrap().code().unwrap_or(1)))
            }
            2 => return Ok(Outcome::OutputFailed),
            _ => {}
        }
        let elapsed = start.elapsed();
        if elapsed >= hard_limit {
            return Ok(Outcome::HardDeadline);
        }
        let last = Duration::from_millis(heartbeat.load(Ordering::Acquire));
        if elapsed.saturating_sub(last) >= heartbeat_limit {
            return Ok(Outcome::HeartbeatExpired);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
