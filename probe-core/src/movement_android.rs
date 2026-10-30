use crate::movement::{Command, Conditions, MovementTask, ScreenFrame, HEIGHT, WIDTH};
use std::{ffi::c_void, sync::Mutex};

static TASK: Mutex<Option<MovementTask>> = Mutex::new(None);

#[no_mangle]
pub extern "system" fn Java_dev_paimon_probe_MovementNative_start(
    _: *mut c_void,
    _: *mut c_void,
    now: i64,
) {
    if let Ok(mut task) = TASK.lock() {
        *task = (now >= 0).then(|| MovementTask::start(now as u64));
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_MovementNative_step(
    env: *mut c_void,
    _: *mut c_void,
    now: i64,
    condition: i32,
    captured_at: i64,
    buffer: *mut c_void,
) -> i32 {
    let conditions = match condition {
        0 => Conditions::Ready,
        1 => Conditions::UserStopped,
        2 => Conditions::PermissionLost,
        3 => Conditions::ScreenLost,
        4 => Conditions::WrongForeground,
        5 => Conditions::DisplayChanged,
        _ => Conditions::Locked,
    };
    let Ok(mut slot) = TASK.lock() else { return 19 };
    let Some(task) = slot.as_mut() else { return 13 };
    if now < 0 || captured_at < 0 {
        return 15;
    }
    let frame = if buffer.is_null() {
        None
    } else {
        // JNI 1.4 table slots, defined by the platform jni.h. The Kotlin caller
        // owns this direct buffer for the duration of the synchronous call.
        let functions = *(env as *const *const *const c_void);
        let address: unsafe extern "system" fn(*mut c_void, *mut c_void) -> *const u8 =
            std::mem::transmute(*functions.add(230));
        let capacity: unsafe extern "system" fn(*mut c_void, *mut c_void) -> i64 =
            std::mem::transmute(*functions.add(231));
        let bytes = address(env, buffer);
        if bytes.is_null() || capacity(env, buffer) != (WIDTH * HEIGHT * 3) as i64 {
            return match task.advance(now as u64, Conditions::ScreenLost, None) {
                Command::End(verdict) => verdict as i32,
                _ => 15,
            };
        }
        Some(ScreenFrame {
            captured_at: captured_at as u64,
            rgb: std::slice::from_raw_parts(bytes, WIDTH * HEIGHT * 3),
        })
    };
    match task.advance(now as u64, conditions, frame) {
        Command::Observe => 0,
        Command::Move => 1,
        Command::End(verdict) => verdict as i32,
    }
}

#[no_mangle]
pub extern "system" fn Java_dev_paimon_probe_MovementNative_inputFinished(
    _: *mut c_void,
    _: *mut c_void,
    now: i64,
    success: u8,
) {
    if let Ok(mut slot) = TASK.lock() {
        if let Some(task) = slot.as_mut() {
            if now >= 0 {
                task.input_finished(now as u64, success != 0);
            }
        }
    }
}
