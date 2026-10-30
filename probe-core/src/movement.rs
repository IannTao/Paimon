//! One bounded movement task, observed through RGB screen frames and input results.
pub const WIDTH: usize = 496;
pub const HEIGHT: usize = 224;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conditions {
    Ready,
    UserStopped,
    PermissionLost,
    ScreenLost,
    WrongForeground,
    DisplayChanged,
    Locked,
}

pub struct ScreenFrame<'a> {
    pub captured_at: u64,
    pub rgb: &'a [u8],
}

#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    MovementConfirmed = 10,
    NoMovementEvidence = 11,
    InvalidScreen = 12,
    UserStopped = 13,
    PermissionLost = 14,
    ScreenLost = 15,
    WrongForeground = 16,
    DisplayChanged = 17,
    Locked = 18,
    TimedOut = 19,
    UnstableBaseline = 20,
    InputFailed = 21,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Observe,
    Move,
    End(Verdict),
}

enum Stage {
    Baseline,
    Stable,
    Moving,
    Verify,
    Confirm,
    Ended(Verdict),
}

pub struct MovementTask {
    stage: Stage,
    baseline: Vec<u8>,
    next_frame_at: u64,
    displacement: (i32, i32),
    deadline: u64,
    last_frame_at: u64,
    last_now: u64,
}

impl MovementTask {
    pub fn start(now: u64) -> Self {
        Self {
            stage: Stage::Baseline,
            baseline: Vec::new(),
            next_frame_at: now,
            displacement: (0, 0),
            deadline: now.saturating_add(8_000),
            last_frame_at: now,
            last_now: now,
        }
    }

    pub fn advance(
        &mut self,
        now: u64,
        conditions: Conditions,
        frame: Option<ScreenFrame<'_>>,
    ) -> Command {
        if let Stage::Ended(verdict) = self.stage {
            return Command::End(verdict);
        }
        let interruption = match conditions {
            Conditions::Ready => None,
            Conditions::UserStopped => Some(Verdict::UserStopped),
            Conditions::PermissionLost => Some(Verdict::PermissionLost),
            Conditions::ScreenLost => Some(Verdict::ScreenLost),
            Conditions::WrongForeground => Some(Verdict::WrongForeground),
            Conditions::DisplayChanged => Some(Verdict::DisplayChanged),
            Conditions::Locked => Some(Verdict::Locked),
        };
        if let Some(verdict) = interruption {
            return self.end(verdict);
        }
        if now < self.last_now || now >= self.deadline {
            return self.end(Verdict::TimedOut);
        }
        self.last_now = now;
        if let Some(frame) = frame.as_ref() {
            if frame.captured_at > now
                || now - frame.captured_at > 1_500
                || frame.captured_at < self.last_frame_at
            {
                return self.end(Verdict::ScreenLost);
            }
            self.last_frame_at = frame.captured_at;
        } else if now.saturating_sub(self.last_frame_at) > 1_500 {
            return self.end(Verdict::ScreenLost);
        }
        let Some(frame) = frame else {
            return Command::Observe;
        };
        if !gameplay_frame(frame.rgb) {
            return self.end(Verdict::InvalidScreen);
        }
        if frame.captured_at < self.next_frame_at {
            return Command::Observe;
        }
        match self.stage {
            Stage::Baseline => {
                self.baseline = frame.rgb.to_vec();
                self.next_frame_at = now + 300;
                self.stage = Stage::Stable;
            }
            Stage::Stable => {
                if map_error(&self.baseline, frame.rgb, 0, 0) > 8_000 {
                    return self.end(Verdict::UnstableBaseline);
                }
                self.stage = Stage::Moving;
                return Command::Move;
            }
            Stage::Moving => {}
            Stage::Verify => {
                if let Some(displacement) = translation(&self.baseline, frame.rgb) {
                    self.displacement = displacement;
                    self.stage = Stage::Confirm;
                    self.next_frame_at = now + 300;
                } else {
                    return self.end(Verdict::NoMovementEvidence);
                }
            }
            Stage::Confirm => {
                return self.end(
                    if translation(&self.baseline, frame.rgb) == Some(self.displacement) {
                        Verdict::MovementConfirmed
                    } else {
                        Verdict::NoMovementEvidence
                    },
                );
            }
            Stage::Ended(_) => unreachable!(),
        }
        Command::Observe
    }

    pub fn input_finished(&mut self, now: u64, success: bool) {
        if matches!(self.stage, Stage::Moving) {
            if now >= self.deadline || now < self.last_now {
                self.end(Verdict::TimedOut);
                return;
            }
            if !success {
                self.end(Verdict::InputFailed);
                return;
            }
            self.stage = Stage::Verify;
            self.next_frame_at = now + 250;
        }
    }

    fn end(&mut self, verdict: Verdict) -> Command {
        self.stage = Stage::Ended(verdict);
        Command::End(verdict)
    }
}

// Fixed Chinese, landscape layout: require a green health bar and textured minimap.
// A different HUD/low health is deliberately unsupported by this first probe.
fn gameplay_frame(rgb: &[u8]) -> bool {
    if rgb.len() != WIDTH * HEIGHT * 3 {
        return false;
    }
    let green = (210..285)
        .filter(|x| {
            let i = (205 * WIDTH + x) * 3;
            rgb[i + 1] > 100
                && rgb[i + 1] > rgb[i].saturating_add(20)
                && rgb[i + 1] > rgb[i + 2].saturating_add(35)
        })
        .count();
    let (mut low, mut high) = (255u8, 0u8);
    for y in 16..50 {
        for x in 45..80 {
            let value = rgb[(y * WIDTH + x) * 3];
            low = low.min(value);
            high = high.max(value);
        }
    }
    green >= 30 && high.saturating_sub(low) >= 35
}

// Match the map terrain ring, excluding the stationary player arrow and outer HUD.
fn translation(before: &[u8], after: &[u8]) -> Option<(i32, i32)> {
    let mut best = (u64::MAX, 0, 0);
    let mut stationary = 0;
    for dy in -6i32..=6 {
        for dx in -6i32..=6 {
            let error = map_error(before, after, dx, dy);
            if dx == 0 && dy == 0 {
                stationary = error;
            }
            if error < best.0 {
                best = (error, dx, dy);
            }
        }
    }
    (best.1 != 0 || best.2 != 0).then_some(())?;
    (best.0 < 25_000 && best.0 * 5 < stationary * 4).then_some((best.1, best.2))
}

fn map_error(before: &[u8], after: &[u8], dx: i32, dy: i32) -> u64 {
    let mut error = 0;
    for y in 13i32..54 {
        for x in 42i32..83 {
            let radius = (x - 62).pow(2) + (y - 33).pow(2);
            if !(144..=400).contains(&radius) {
                continue;
            }
            for channel in 0..3 {
                let a = before[(y as usize * WIDTH + x as usize) * 3 + channel];
                let b = after[((y + dy) as usize * WIDTH + (x + dx) as usize) * 3 + channel];
                error += u64::from(a.abs_diff(b));
            }
        }
    }
    error
}
