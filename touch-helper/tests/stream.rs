use paimon_touch_helper::{Event as E, Stream, SYN_REPORT};

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
fn reserved_virtual_slot_and_id_cannot_be_overwritten_by_physical_contacts() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    stream.begin_tap(20, 30).unwrap();
    let frame = [
        E::new(3, 47, 0),
        E::new(3, 57, 100),
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
fn incomplete_touch_key_state_disables_pickup_contacts() {
    let mut stream = Stream::new(2, 1000, 2000, 100).unwrap();
    assert!(stream.forward(&[E::new(1, 330, 1), SYN_REPORT]).is_err());
    assert!(stream.begin_tap(10, 20).is_err());
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
            E::new(3, 57, 65536),
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
