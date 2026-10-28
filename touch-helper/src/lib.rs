//! A single serialized Type B stream. No Android or device access in this module.
mod control;
pub mod supervisor;
pub use control::{Command, Gate};
pub const SYN_REPORT: Event = Event::new(0, 0, 0);

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
        })
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
        // One extra virtual slot and ID are never used by physical events.
        let slot = self.fingers.len();
        let id = self.max_id + 1;
        self.synthetic = Some((slot, id));
        Ok(vec![
            Event::new(3, 47, slot as i32),
            Event::new(3, 57, id),
            Event::new(3, 53, x),
            Event::new(3, 54, y),
            Event::new(1, 330, 1),
            Event::new(3, 47, self.selected as i32),
            SYN_REPORT,
        ])
    }

    pub fn end_tap(&mut self) -> Vec<Event> {
        let Some((slot, _)) = self.synthetic.take() else {
            return vec![];
        };
        vec![
            Event::new(3, 47, slot as i32),
            Event::new(3, 57, -1),
            Event::new(1, 330, i32::from(!self.real_fingers_up())),
            Event::new(3, 47, self.selected as i32),
            SYN_REPORT,
        ]
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
        for event in &frame[..frame.len() - 1] {
            match (event.kind, event.code, event.value) {
                (0, _, _) => return Err("lost or unexpected synchronization"),
                (3, 47, slot) => {
                    selected = usize::try_from(slot).map_err(|_| "negative slot")?;
                    if selected >= fingers.len() {
                        return Err("invalid slot");
                    }
                }
                (3, 57, id) => {
                    if id < -1 || id > self.max_id {
                        return Err("invalid tracking ID");
                    }
                    fingers[selected] = Finger {
                        id: (id >= 0).then_some(id),
                    };
                }
                (3, 53, x) if !(0..=self.max_x).contains(&x) => return Err("X out of range"),
                (3, 54, y) if !(0..=self.max_y).contains(&y) => return Err("Y out of range"),
                _ => {}
            }
        }
        for (index, finger) in fingers.iter().enumerate() {
            if finger.id.is_some() && fingers[..index].iter().any(|f| f.id == finger.id) {
                return Err("duplicate tracking ID");
            }
        }
        self.fingers = fingers;
        self.selected = selected;
        Ok(frame
            .iter()
            .map(|event| {
                if self.synthetic.is_some() && (event.kind, event.code) == (1, 330) {
                    Event::new(1, 330, 1)
                } else {
                    *event
                }
            })
            .collect())
    }
}
