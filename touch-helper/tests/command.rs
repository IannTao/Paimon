use paimon_touch_helper::Command;

#[test]
fn experiment_tap_can_require_two_current_real_contacts() {
    assert_eq!(
        Command::parse("tap-if 2 100 200").unwrap(),
        Command::TapIfContacts(2, 100, 200)
    );
    assert!(Command::parse("tap-if 33 100 200").is_err());
}

#[test]
fn pickup_requires_a_live_permit_and_has_a_bounded_count() {
    let mut gate = paimon_touch_helper::Gate::default();
    assert!(!gate.allow_tap(0));
    gate.apply(Command::Permit, 10);
    assert!(gate.allow_tap(10));
    assert!(!gate.allow_tap(1010));
    gate.apply(Command::Permit, 1020);
    assert!(!gate.allow_tap(1020)); // An expired permit permanently drains this session.
}

#[test]
fn thirty_pickup_contacts_are_the_session_limit() {
    let mut gate = paimon_touch_helper::Gate::default();
    for index in 0..30 {
        let now = index * 10;
        gate.apply(Command::Permit, now);
        assert!(gate.allow_tap(now));
        gate.tap_started();
    }
    assert!(!gate.allow_tap(300));
}

#[test]
fn invalid_commands_are_rejected_and_stop_cannot_be_revoked() {
    assert!(Command::parse("tap 12").is_err());
    assert!(Command::parse("tap 2 3 extra").is_err());
    assert!(Command::parse("tap 2147483648 3").is_err());
    assert_eq!(Command::parse("tap 12 34\n").unwrap(), Command::Tap(12, 34));
    let mut gate = paimon_touch_helper::Gate::default();
    gate.apply(Command::Stop, 0);
    gate.apply(Command::Permit, 1);
    assert!(!gate.allow_tap(1));
    assert!(gate.draining());
}
