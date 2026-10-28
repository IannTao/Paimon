//! A single serialized Type B stream. No Android or device access in this module.
pub mod codes;
mod control;
use codes::*;
pub mod supervisor;
pub use control::{Command, Gate};
pub const SYN_REPORT: Event = Event::new(EV_SYN, 0, 0);

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: u16,
    pub code: u16,
    pub value: i32,
}
impl Event {
    pub const fn new(kind: u16, code: u16, value: i32) -> Self {
        Self { kind, code, value }
    }
}

#[derive(Clone, Default)]
struct Finger {
    id: Option<i32>,
}

pub struct Stream {
    fingers: Vec<Finger>,
    selected: usize,
    max_x: i32,
    max_y: i32,
    max_id: i32,
    synthetic: Option<(usize, i32)>,
    healthy: bool,
    real_touch_key: bool,
    tool_finger: bool,
    real_tool_finger: i32,
}

impl Stream {
    /// Start only after an idle MT snapshot has been copied to the virtual device.
    /// The physical device must already be grabbed, with pre-grab events drained.
    pub fn new(slots: usize, max_x: i32, max_y: i32, max_id: i32) -> Result<Self, &'static str> {
        if !(2..=32).contains(&slots)
            || max_x <= 0
            || max_y <= 0
            || max_id < slots as i32
            || max_id == i32::MAX
        {
            return Err("unsupported touch ranges");
        }
        Ok(Self {
            fingers: vec![Finger::default(); slots],
            selected: 0,
            max_x,
            max_y,
            max_id,
            synthetic: None,
            healthy: true,
            real_touch_key: false,
            tool_finger: false,
            real_tool_finger: 0,
        })
    }

    pub fn with_tool_finger(mut self) -> Self {
        self.tool_finger = true;
        self
    }

    pub fn real_fingers_up(&self) -> bool {
        self.fingers.iter().all(|finger| finger.id.is_none())
    }

    pub fn begin_tap(&mut self, x: i32, y: i32) -> Result<Vec<Event>, &'static str> {
        if !self.healthy {
            return Err("stream state is unknown");
        }
        if self.synthetic.is_some() {
            return Err("tap already active");
        }
        if !(0..=self.max_x).contains(&x) || !(0..=self.max_y).contains(&y) {
            return Err("tap outside raw axis ranges");
        }
        if self.fingers.iter().all(|f| f.id.is_some()) {
            return Err("all slots occupied");
        }
        // The virtual slot is reserved; the ID must remain within advertised
        // ranges because Android/kernel may normalize the tracking-ID maximum.
        let slot = self.fingers.len();
        let id = (0..=self.max_id)
            .rev()
            .find(|id| self.fingers.iter().all(|finger| finger.id != Some(*id)))
            .ok_or("no free tracking ID")?;
        self.synthetic = Some((slot, id));
        let mut output = vec![
            Event::new(EV_ABS, ABS_MT_SLOT, slot as i32),
            Event::new(EV_ABS, ABS_MT_TRACKING_ID, id),
            Event::new(EV_ABS, ABS_MT_POSITION_X, x),
            Event::new(EV_ABS, ABS_MT_POSITION_Y, y),
            Event::new(EV_KEY, BTN_TOUCH, 1),
            Event::new(EV_ABS, ABS_MT_SLOT, self.selected as i32),
            SYN_REPORT,
        ];
        if self.tool_finger {
            output.insert(
                output.len() - 1,
                Event::new(EV_KEY, BTN_TOOL_FINGER, i32::from(self.real_fingers_up())),
            );
        }
        Ok(output)
    }

    pub fn end_tap(&mut self) -> Vec<Event> {
        let Some((slot, _)) = self.synthetic.take() else {
            return vec![];
        };
        let mut output = vec![
            Event::new(EV_ABS, ABS_MT_SLOT, slot as i32),
            Event::new(EV_ABS, ABS_MT_TRACKING_ID, -1),
            Event::new(EV_KEY, BTN_TOUCH, i32::from(!self.real_fingers_up())),
            Event::new(EV_ABS, ABS_MT_SLOT, self.selected as i32),
            SYN_REPORT,
        ];
        if self.tool_finger {
            output.insert(
                output.len() - 1,
                Event::new(EV_KEY, BTN_TOOL_FINGER, self.real_tool_finger),
            );
        }
        output
    }

    pub fn forward(&mut self, frame: &[Event]) -> Result<Vec<Event>, &'static str> {
        if !self.healthy {
            return Err("stream state is unknown");
        }
        let result = self.forward_checked(frame);
        if result.is_err() {
            self.healthy = false;
        }
        result
    }

    fn forward_checked(&mut self, frame: &[Event]) -> Result<Vec<Event>, &'static str> {
        if frame.last() != Some(&SYN_REPORT) || frame.len() > 4096 {
            return Err("incomplete or oversized frame");
        }
        let mut fingers = self.fingers.clone();
        let mut selected = self.selected;
        let mut touch_key = self.real_touch_key;
        let mut tool_finger = self.real_tool_finger;
        for event in &frame[..frame.len() - 1] {
            match (event.kind, event.code, event.value) {
                (EV_SYN, _, _) => return Err("lost or unexpected synchronization"),
                (EV_KEY, BTN_TOUCH, value @ (0 | 1)) => touch_key = value == 1,
                (EV_KEY, BTN_TOUCH, _) => return Err("invalid touch key state"),
                (EV_KEY, BTN_TOOL_FINGER, value @ (0 | 1)) if self.tool_finger => {
                    tool_finger = value
                }
                (EV_KEY, BTN_TOOL_FINGER, _) => {
                    return Err("unsupported or invalid finger tool state")
                }
                (EV_ABS, ABS_MT_SLOT, slot) => {
                    selected = usize::try_from(slot).map_err(|_| "negative slot")?;
                    if selected >= fingers.len() {
                        return Err("invalid slot");
                    }
                }
                (EV_ABS, ABS_MT_TRACKING_ID, id) => {
                    if id < -1 || id > self.max_id {
                        return Err("invalid tracking ID");
                    }
                    fingers[selected] = Finger {
                        id: (id >= 0).then_some(id),
                    };
                }
                (EV_ABS, ABS_MT_POSITION_X, x) if !(0..=self.max_x).contains(&x) => {
                    return Err("X out of range")
                }
                (EV_ABS, ABS_MT_POSITION_Y, y) if !(0..=self.max_y).contains(&y) => {
                    return Err("Y out of range")
                }
                _ => {}
            }
        }
        for (index, finger) in fingers.iter().enumerate() {
            if finger.id.is_some() && fingers[..index].iter().any(|f| f.id == finger.id) {
                return Err("duplicate tracking ID");
            }
        }
        if touch_key != fingers.iter().any(|f| f.id.is_some()) {
            return Err("touch key and MT slots disagree");
        }
        let mut prefix = vec![];
        if self.synthetic.is_some_and(|(_, id)| {
            frame.iter().any(|event| {
                (event.kind, event.code, event.value) == (EV_ABS, ABS_MT_TRACKING_ID, id)
            })
        }) {
            // Finish the pickup before forwarding the conflicting real DOWN.
            // Use the previous real state for this release frame.
            prefix = self.end_tap();
        }
        self.fingers = fingers;
        self.selected = selected;
        self.real_touch_key = touch_key;
        self.real_tool_finger = tool_finger;
        let mut output: Vec<_> = frame
            .iter()
            .map(|event| {
                if self.synthetic.is_some() && (event.kind, event.code) == (EV_KEY, BTN_TOUCH) {
                    Event::new(EV_KEY, BTN_TOUCH, 1)
                } else if self.synthetic.is_some()
                    && (event.kind, event.code) == (EV_KEY, BTN_TOOL_FINGER)
                {
                    Event::new(EV_KEY, BTN_TOOL_FINGER, i32::from(self.real_fingers_up()))
                } else {
                    *event
                }
            })
            .collect();
        if self.synthetic.is_some() && self.tool_finger {
            output.insert(
                output.len() - 1,
                Event::new(EV_KEY, BTN_TOOL_FINGER, i32::from(self.real_fingers_up())),
            );
        }
        prefix.extend(output);
        Ok(prefix)
    }
}
