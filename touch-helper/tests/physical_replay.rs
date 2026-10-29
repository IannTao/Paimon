use paimon_touch_helper::{Event, Stream, SYN_REPORT};

#[test]
fn captured_physical_gestures_forward_without_losing_state() {
    let capture = include_str!(
        "../../docs/validation/reports/touch-helper-2026-10-28/physical-raw-20261029.txt"
    );
    let mut stream = Stream::new(10, 17856, 39680, 65535)
        .unwrap()
        .with_tool_finger();
    let mut frame = Vec::new();
    let mut frames = 0;
    for line in capture.lines().filter(|line| line.starts_with('[')) {
        let columns: Vec<_> = line.split_whitespace().collect();
        let fields = &columns[columns.len() - 3..];
        let event = Event::new(
            u16::from_str_radix(fields[0], 16).unwrap(),
            u16::from_str_radix(fields[1], 16).unwrap(),
            u32::from_str_radix(fields[2], 16).unwrap() as i32,
        );
        frame.push(event);
        if event == SYN_REPORT {
            frames += 1;
            assert_eq!(
                stream.forward(&frame).unwrap_or_else(|error| {
                    panic!("frame {frames}: {error}; events={frame:?}")
                }),
                frame
            );
            frame.clear();
        }
    }
    assert!(frames > 100, "capture must contain real gestures");
    assert!(frame.is_empty(), "capture ended inside a frame");
    assert!(stream.real_fingers_up());
}
