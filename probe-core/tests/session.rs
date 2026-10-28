use paimon_probe::{Action, EndReason, Point, Session};

#[test]
fn a_command_completing_after_the_session_deadline_cannot_complete_the_experiment() {
    let target = Point { x: 2000, y: 500 };
    let mut session = Session::start(2480, 1116, target, 2, 0).unwrap();
    assert_eq!(session.poll(5_000), Action::Click(target));
    session.acknowledge(5_010, true);
    assert_eq!(session.poll(6_010), Action::Click(target));
    session.acknowledge(120_001, true);
    assert_eq!(session.poll(120_002), Action::Ended(EndReason::Expired));
}

#[test]
fn an_explicitly_started_session_counts_down_and_completes_only_its_requested_clicks() {
    let target = Point { x: 2000, y: 500 };
    let mut session = Session::start(2480, 1116, target, 2, 10_000).unwrap();
    assert_eq!(session.poll(14_999), Action::Waiting);
    assert_eq!(session.poll(15_000), Action::Click(target));
    assert_eq!(session.poll(15_500), Action::Waiting);
    session.acknowledge(15_600, true);
    assert_eq!(session.poll(16_599), Action::Waiting);
    assert_eq!(session.poll(16_600), Action::Click(target));
    session.acknowledge(16_700, true);
    // Command completion has its own label, never a verdict about physical touch.
    assert_eq!(
        session.poll(17_700),
        Action::Ended(EndReason::SequenceComplete)
    );
    assert_eq!(
        session.poll(30_000),
        Action::Ended(EndReason::SequenceComplete)
    );
}

#[test]
fn a_slow_command_does_not_accumulate_a_burst_of_overdue_clicks() {
    let target = Point { x: 2000, y: 500 };
    let mut session = Session::start(2480, 1116, target, 3, 0).unwrap();
    assert_eq!(session.poll(5_000), Action::Click(target));
    assert_eq!(session.poll(50_000), Action::Waiting);
    session.acknowledge(50_000, true);
    assert_eq!(session.poll(50_999), Action::Waiting);
    assert_eq!(session.poll(51_000), Action::Click(target));
    assert_eq!(session.poll(51_000), Action::Waiting);
}

#[test]
fn stopping_during_the_countdown_prevents_all_clicks() {
    let mut session = Session::start(2480, 1116, Point { x: 2000, y: 500 }, 10, 0).unwrap();
    session.stop();
    assert_eq!(session.poll(5_000), Action::Ended(EndReason::UserStopped));
    assert_eq!(session.poll(90_000), Action::Ended(EndReason::UserStopped));
}

#[test]
fn a_late_success_after_stop_cannot_restart_the_session() {
    let target = Point { x: 2000, y: 500 };
    let mut session = Session::start(2480, 1116, target, 10, 0).unwrap();
    assert_eq!(session.poll(5_000), Action::Click(target));
    session.stop();
    session.acknowledge(5_100, true);
    assert_eq!(session.poll(6_100), Action::Ended(EndReason::UserStopped));
}

#[test]
fn an_input_command_failure_ends_the_session_without_retries() {
    let target = Point { x: 2000, y: 500 };
    let mut session = Session::start(2480, 1116, target, 10, 0).unwrap();
    assert_eq!(session.poll(5_000), Action::Click(target));
    session.acknowledge(5_100, false);
    assert_eq!(session.poll(6_100), Action::Ended(EndReason::CommandFailed));
}

#[test]
fn a_session_expires_even_if_a_command_never_reports_completion() {
    let target = Point { x: 2000, y: 500 };
    let mut session = Session::start(2480, 1116, target, 10, 0).unwrap();
    assert_eq!(session.poll(5_000), Action::Click(target));
    assert_eq!(session.poll(119_999), Action::Waiting);
    assert_eq!(session.poll(120_000), Action::Ended(EndReason::Expired));
}

#[test]
fn invalid_coordinates_or_unbounded_repetitions_cannot_start_a_session() {
    for (width, height, point, count) in [
        (1116, 2480, Point { x: 500, y: 500 }, 10),
        (2480, 0, Point { x: 500, y: 0 }, 10),
        (2480, 1116, Point { x: 2480, y: 500 }, 10),
        (2480, 1116, Point { x: 2000, y: 1116 }, 10),
        (2480, 1116, Point { x: 2000, y: 500 }, 0),
        (2480, 1116, Point { x: 2000, y: 500 }, 31),
    ] {
        assert!(Session::start(width, height, point, count, 0).is_err());
    }
}
