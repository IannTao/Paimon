//! The supervisor owns no evdev/uinput descriptors. A stuck worker is killed locally.
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Exited(i32),
    HeartbeatExpired,
    HardDeadline,
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
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            let Ok(count) = reader.by_ref().take(16385).read_line(&mut line) else {
                break;
            };
            if count == 0 || count > 16384 {
                break;
            }
            if line.trim() == "{\"event\":\"heartbeat\"}" {
                received.store(start.elapsed().as_millis() as u64, Ordering::Release);
            } else if io::stdout().write_all(line.as_bytes()).is_err() {
                break;
            }
        }
    });
    loop {
        if let Some(status) = worker.0.try_wait()? {
            return Ok(Outcome::Exited(status.code().unwrap_or(1)));
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
