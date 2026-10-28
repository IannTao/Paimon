use paimon_touch_helper::supervisor::{supervise, Outcome};
use std::{
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn a_hung_worker_is_killed_without_a_host_connection() {
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args(["--ignored", "--exact", "hung_worker", "--nocapture"]);
    let start = Instant::now();
    assert_eq!(
        supervise(child, Duration::from_secs(5), Duration::from_millis(200)).unwrap(),
        Outcome::HeartbeatExpired
    );
    assert!(start.elapsed() < Duration::from_secs(4));
}

#[test]
#[ignore]
fn hung_worker() {
    std::thread::sleep(Duration::from_secs(30));
}

#[test]
fn healthy_heartbeats_cannot_extend_the_hard_deadline() {
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args(["--ignored", "--exact", "healthy_worker", "--nocapture"]);
    assert_eq!(
        supervise(child, Duration::from_millis(300), Duration::from_secs(2)).unwrap(),
        Outcome::HardDeadline
    );
}

#[test]
#[ignore]
fn healthy_worker() {
    use std::io::Write;
    loop {
        println!("{{\"event\":\"heartbeat\"}}");
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_millis(30));
    }
}
