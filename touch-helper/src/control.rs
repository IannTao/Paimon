#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Permit,
    Tap(i32, i32),
    TapIfContacts(usize, i32, i32),
    Stop,
}

impl Command {
    pub fn parse(line: &str) -> Result<Self, &'static str> {
        let words: Vec<_> = line.split_whitespace().collect();
        match words.as_slice() {
            ["permit"] => Ok(Self::Permit),
            ["stop"] => Ok(Self::Stop),
            ["tap", x, y] => Ok(Self::Tap(
                x.parse().map_err(|_| "invalid X")?,
                y.parse().map_err(|_| "invalid Y")?,
            )),
            ["tap-if", count, x, y] => {
                let count: usize = count.parse().map_err(|_| "invalid contact count")?;
                if count > 32 {
                    return Err("contact count must be 0..32");
                }
                Ok(Self::TapIfContacts(
                    count,
                    x.parse().map_err(|_| "invalid X")?,
                    y.parse().map_err(|_| "invalid Y")?,
                ))
            }
            _ => Err("expected permit, tap RAW_X RAW_Y, tap-if COUNT RAW_X RAW_Y, or stop"),
        }
    }
}

/// The caller renews permission only after its foreground/recognition checks pass.
/// Expiry or stop is irreversible: keep forwarding until the real fingers lift.
#[derive(Default)]
pub struct Gate {
    permit_until: Option<u64>,
    draining: bool,
    count: u8,
}

impl Gate {
    pub fn apply(&mut self, command: Command, now_ms: u64) {
        self.expire(now_ms);
        match command {
            Command::Stop => self.draining = true,
            Command::Permit if !self.draining => {
                self.permit_until = Some(now_ms.saturating_add(1000))
            }
            _ => {}
        }
    }

    pub fn expire(&mut self, now_ms: u64) {
        if self.permit_until.is_some_and(|until| now_ms >= until) {
            self.draining = true;
        }
    }

    pub fn allow_tap(&mut self, now_ms: u64) -> bool {
        self.expire(now_ms);
        !self.draining && self.permit_until.is_some() && self.count < 30
    }

    pub fn tap_started(&mut self) {
        self.count += 1;
    }
    pub fn draining(&self) -> bool {
        self.draining
    }
}
