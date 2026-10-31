//! A bounded, explicitly started touch experiment. Command completion is not
//! evidence that the game accepted a click or that physical touches survived.

pub mod movement;

#[cfg(target_os = "android")]
mod movement_android;

#[cfg(target_os = "android")]
mod route_android;

const COUNTDOWN_MS: u64 = 5_000;
const INTERVAL_MS: u64 = 1_000;
const MAX_SESSION_MS: u64 = 120_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    SequenceComplete,
    UserStopped,
    CommandFailed,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Waiting,
    Click(Point),
    Ended(EndReason),
}

pub struct Session {
    point: Point,
    remaining: u32,
    due: u64,
    deadline: u64,
    pending: bool,
    ended: Option<EndReason>,
}

impl Session {
    pub fn start(
        width: u32,
        height: u32,
        point: Point,
        count: u32,
        now: u64,
    ) -> Result<Self, &'static str> {
        if width <= height || width > 16_384 || height == 0 || point.x >= width || point.y >= height
        {
            return Err("target must be inside the configured landscape display");
        }
        if !(2..=30).contains(&count) {
            return Err("click count must be between 2 and 30");
        }
        Ok(Self {
            point,
            remaining: count,
            due: now.saturating_add(COUNTDOWN_MS),
            deadline: now.saturating_add(MAX_SESSION_MS),
            pending: false,
            ended: None,
        })
    }

    pub fn poll(&mut self, now: u64) -> Action {
        if let Some(reason) = self.ended {
            return Action::Ended(reason);
        }
        if now >= self.deadline {
            self.ended = Some(EndReason::Expired);
            return Action::Ended(EndReason::Expired);
        }
        if self.pending || now < self.due {
            return Action::Waiting;
        }
        self.pending = true;
        Action::Click(self.point)
    }

    pub fn acknowledge(&mut self, now: u64, command_succeeded: bool) {
        if self.ended.is_some() || !self.pending {
            return;
        }
        if now >= self.deadline {
            self.ended = Some(EndReason::Expired);
            self.pending = false;
            return;
        }
        self.pending = false;
        if !command_succeeded {
            self.ended = Some(EndReason::CommandFailed);
        } else {
            self.remaining -= 1;
            if self.remaining == 0 {
                self.ended = Some(EndReason::SequenceComplete);
            } else {
                // No backlog or catch-up bursts after a slow command.
                self.due = now.saturating_add(INTERVAL_MS);
            }
        }
    }

    pub fn stop(&mut self) {
        if self.ended.is_none() {
            self.ended = Some(EndReason::UserStopped);
        }
    }
}

#[cfg(target_os = "android")]
mod android {
    use super::{Action, Point, Session};
    use std::{ffi::c_void, sync::Mutex};

    static SESSION: Mutex<Option<Session>> = Mutex::new(None);

    #[no_mangle]
    pub extern "system" fn Java_dev_paimon_probe_ProbeNative_start(
        _env: *mut c_void,
        _class: *mut c_void,
        now: i64,
        width: i32,
        height: i32,
        x: i32,
        y: i32,
        count: i32,
    ) -> u8 {
        let Ok(mut slot) = SESSION.lock() else {
            return 0;
        };
        *slot = None;
        if now < 0 || width < 0 || height < 0 || x < 0 || y < 0 || count < 0 {
            return 0;
        }
        let result = Session::start(
            width as u32,
            height as u32,
            Point {
                x: x as u32,
                y: y as u32,
            },
            count as u32,
            now as u64,
        );
        match result {
            Ok(session) => {
                *slot = Some(session);
                1
            }
            Err(_) => 0,
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_paimon_probe_ProbeNative_next(
        _env: *mut c_void,
        _class: *mut c_void,
        now: i64,
    ) -> i64 {
        let Ok(mut slot) = SESSION.lock() else {
            return -1;
        };
        let Some(session) = slot.as_mut() else {
            return -1;
        };
        if now < 0 {
            session.stop();
            return -1;
        }
        match session.poll(now as u64) {
            Action::Waiting => 0,
            Action::Click(point) => ((point.x as i64) << 32 | point.y as i64) + 1,
            Action::Ended(super::EndReason::Expired) => -2,
            Action::Ended(_) => -1,
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_paimon_probe_ProbeNative_acknowledge(
        _env: *mut c_void,
        _class: *mut c_void,
        now: i64,
        success: u8,
    ) {
        if let Ok(mut slot) = SESSION.lock() {
            if let Some(session) = slot.as_mut() {
                if now < 0 {
                    session.stop()
                } else {
                    session.acknowledge(now as u64, success != 0)
                }
            }
        }
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_paimon_probe_ProbeNative_stop(
        _env: *mut c_void,
        _class: *mut c_void,
    ) {
        if let Ok(mut slot) = SESSION.lock() {
            if let Some(session) = slot.as_mut() {
                session.stop()
            }
        }
    }
}
pub mod route_import;
