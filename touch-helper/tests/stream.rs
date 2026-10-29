use paimon_touch_helper::{Event as E, Stream, SYN_REPORT};

#[test]
fn third_contact_test_does_not_click_after_a_real_finger_has_lifted() {
    let mut stream = Stream::new(3, 1000, 2000, 100).unwrap();
    stream
        .forward(&[E::new(3, 57, 7), E::new(1, 330, 1), SYN_REPORT])
        .unwrap();
    assert!(stream.begin_tap_if_contacts(2, 300, 400).is_err());
    assert!(stream.end_tap().is_empty());
    stream
        .forward(&[E::new(3, 47, 1), E::new(3, 57, 8), SYN_REPORT])
        .unwrap();
    assert!(stream.begin_tap_if_contacts(2, 300, 400).is_ok());
}

#[test]
fn pickup_id_stays_in_range_and_yields_to_a_real_tracking_id() {
    let mut stream = Stream::new(3, 1000, 2000, 100).unwrap();
    stream
        .forward(&[E::new(3, 57, 7), E::new(1, 330, 1), SYN_REPORT])
        .unwrap();
    assert!(stream
        .begin_tap(30, 40)
        .unwrap()
        .contains(&E::new(3, 57, 100)));
    let frame = [E::new(3, 47, 1), E::new(3, 57, 100), SYN_REPORT];
    let mut expected = vec![
        E::new(3, 47, 3),
        E::new(3, 57, -1),
        E::new(1, 330, 1),
        E::new(3, 47, 0),
        SYN_REPORT,
    ];
    expected.extend(frame);
    assert_eq!(stream.forward(&frame).unwrap(), expected);
    assert!(stream.end_tap().is_empty());
}

#[test]
fn pickup_updates_single_finger_tool_key_and_restores_physical_key_on_release() {
    let mut stream = Stream::new(3, 1000, 2000, 100).unwrap().with_tool_finger();
    let real_down = [
        E::new(3, 57, 7),
        E::new(1, 325, 1),
        E::new(1, 330, 1),
        SYN_REPORT,
    ];
    assert_eq!(stream.forward(&real_down).unwrap(), real_down);
    assert!(stream
        .begin_tap(300, 400)
        .unwrap()
        .contains(&E::new(1, 325, 0)));
    assert!(stream.end_tap().contains(&E::new(1, 325, 1)));
}

#[test]
fn last_real_finger_up_leaves_the_synthetic_finger_tool_present() {
    let mut stream = Stream::new(3, 1000, 2000, 100).unwrap().with_tool_finger();
    stream
        .forward(&[
            E::new(3, 57, 7),
            E::new(1, 325, 1),
            E::new(1, 330, 1),
            SYN_REPORT,
        ])
        .unwrap();
    stream.begin_tap(300, 400).unwrap();
    let output = stream
        .forward(&[
            E::new(3, 57, -1),
            E::new(1, 325, 0),
            E::new(1, 330, 0),
            SYN_REPORT,
        ])
        .unwrap();
    assert!(output
        .iter()
        .filter(|e| e.kind == 1 && e.code == 325)
        .all(|e| e.value == 1));
    assert!(stream.end_tap().contains(&E::new(1, 325, 0)));
}

#[test]
fn real_two_finger_frame_is_forwarded_unchanged() {
    let mut stream = Stream::new(3, 1000, 2000, 65535).unwrap();
    let frame = vec![
        E::new(3, 47, 0),
        E::new(3, 57, 31),
        E::new(3, 53, 100),
        E::new(3, 54, 200),
        E::new(3, 47, 1),
        E::new(3, 57, 32),
        E::new(3, 53, 300),
        E::new(3, 54, 400),
        E::new(1, 330, 1),
        SYN_REPORT,
    ];
    assert_eq!(stream.forward(&frame).unwrap(), frame);
    assert!(!stream.real_fingers_up());
}

#[test]
fn exhausted_slots_and_out_of_range_pickup_coordinates_are_rejected() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    assert!(stream.begin_tap(1001, 20).is_err());
    stream
        .forward(&[
            E::new(3, 57, 10),
            E::new(3, 47, 1),
            E::new(3, 57, 11),
            E::new(1, 330, 1),
            SYN_REPORT,
        ])
        .unwrap();
    assert!(stream.begin_tap(100, 200).is_err());
    assert!(stream.end_tap().is_empty());
}

#[test]
fn reserved_virtual_slot_stays_separate_from_physical_contacts() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    stream.begin_tap(20, 30).unwrap();
    let frame = [
        E::new(3, 47, 0),
        E::new(3, 57, 98),
        E::new(3, 47, 1),
        E::new(3, 57, 99),
        E::new(1, 330, 1),
        SYN_REPORT,
    ];
    assert_eq!(stream.forward(&frame).unwrap(), frame);
    assert_eq!(
        stream.end_tap(),
        vec![
            E::new(3, 47, 2),
            E::new(3, 57, -1),
            E::new(1, 330, 1),
            E::new(3, 47, 1),
            SYN_REPORT
        ]
    );
}

#[test]
fn cached_idle_coordinates_need_not_be_repeated_on_next_down() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    // Native startup has copied the complete inactive-slot snapshot into uinput.
    let frame = [E::new(3, 57, 7), E::new(1, 330, 1), SYN_REPORT];
    assert_eq!(stream.forward(&frame).unwrap(), frame);
}

#[test]
fn lost_stream_never_accepts_another_pickup_contact() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    assert!(stream.forward(&[E::new(0, 3, 0), SYN_REPORT]).is_err());
    assert!(stream.begin_tap(10, 20).is_err());
}

#[test]
fn captured_key_only_frame_keeps_forwarding_but_blocks_pickup_until_consistent() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    // Captured on goodix_ts after all slots had been released, 2026-10-29.
    let key_only = [E::new(1, 330, 1), SYN_REPORT];
    assert_eq!(stream.forward(&key_only).unwrap(), key_only);
    assert!(stream.begin_tap(10, 20).is_err());
    let real_down = [E::new(3, 57, 7), SYN_REPORT];
    assert_eq!(stream.forward(&real_down).unwrap(), real_down);
    assert!(stream.begin_tap(10, 20).is_ok());
}

#[test]
fn key_slot_disagreement_releases_pickup_before_forwarding_the_source_frame() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    stream
        .forward(&[E::new(3, 57, 7), E::new(1, 330, 1), SYN_REPORT])
        .unwrap();
    stream.begin_tap(10, 20).unwrap();
    let key_up = [E::new(1, 330, 0), SYN_REPORT];
    assert_eq!(
        stream.forward(&key_up).unwrap(),
        vec![
            E::new(3, 47, 2),
            E::new(3, 57, -1),
            E::new(1, 330, 1),
            E::new(3, 47, 0),
            SYN_REPORT,
            E::new(1, 330, 0),
            SYN_REPORT,
        ]
    );
    assert!(stream.end_tap().is_empty());
    assert!(stream.begin_tap(10, 20).is_err());
    assert_eq!(stream.real_contact_count(), 1);
}

#[test]
fn pickup_contact_does_not_release_real_fingers_and_restores_selected_slot() {
    let mut stream = Stream::new(3, 1000, 2000, 65535).unwrap();
    stream
        .forward(&[
            E::new(3, 57, 7),
            E::new(3, 53, 100),
            E::new(3, 54, 200),
            E::new(1, 330, 1),
            SYN_REPORT,
        ])
        .unwrap();
    assert_eq!(
        stream.begin_tap(700, 800).unwrap(),
        vec![
            E::new(3, 47, 3),
            E::new(3, 57, 65535),
            E::new(3, 53, 700),
            E::new(3, 54, 800),
            E::new(1, 330, 1),
            E::new(3, 47, 0),
            SYN_REPORT,
        ]
    );
    assert_eq!(
        stream.end_tap(),
        vec![
            E::new(3, 47, 3),
            E::new(3, 57, -1),
            E::new(1, 330, 1),
            E::new(3, 47, 0),
            SYN_REPORT,
        ]
    );
    assert_eq!(
        stream.forward(&[E::new(3, 53, 110), SYN_REPORT]).unwrap(),
        vec![E::new(3, 53, 110), SYN_REPORT]
    );
    assert!(!stream.real_fingers_up());
}

#[test]
fn last_real_finger_up_keeps_synthetic_contact_down() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    stream
        .forward(&[
            E::new(3, 57, 7),
            E::new(3, 53, 10),
            E::new(3, 54, 20),
            E::new(1, 330, 1),
            SYN_REPORT,
        ])
        .unwrap();
    stream.begin_tap(700, 800).unwrap();
    assert_eq!(
        stream
            .forward(&[E::new(3, 57, -1), E::new(1, 330, 0), SYN_REPORT])
            .unwrap(),
        vec![E::new(3, 57, -1), E::new(1, 330, 1), SYN_REPORT]
    );
    assert_eq!(stream.end_tap()[2], E::new(1, 330, 0));
}
