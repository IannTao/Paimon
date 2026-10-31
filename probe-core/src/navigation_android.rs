use crate::{
    android_jni::{direct, output},
    localization::{MiniMapFrame, MINI_SIZE},
    movement::Conditions,
    navigation::{NavigationCommand, NavigationScope, NavigationState, NavigationTask},
};
use std::{ffi::c_void, sync::Mutex};

static TASK: Mutex<Option<(i64, NavigationTask)>> = Mutex::new(None);
fn scope(first: i32, last: i32) -> NavigationScope {
    if first == 0 && last == 0 {
        NavigationScope::WholeRoute
    } else {
        NavigationScope::Segment {
            first: first.max(0) as usize,
            last: last.max(0) as usize,
        }
    }
}
fn payload(task: &NavigationTask, command: NavigationCommand) -> serde_json_lenient::Value {
    serde_json_lenient::json!({"task":task.report(),"command":command})
}

#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_NavigationNative_plan(
    env: *mut c_void,
    _: *mut c_void,
    bytes: *mut c_void,
    first: i32,
    last: i32,
) -> *mut c_void {
    let Some(bytes) = direct(env, bytes, crate::route_import::MAX_BYTES) else {
        return std::ptr::null_mut();
    };
    match NavigationTask::prepare("selected-route.json", bytes, scope(first, last), 0) {
        Ok(task) => output(env, &payload(&task, NavigationCommand::Observe)),
        Err(error) => output(env, &serde_json_lenient::json!({"error":error})),
    }
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_NavigationNative_start(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    bytes: *mut c_void,
    first: i32,
    last: i32,
    now: i64,
) -> *mut c_void {
    if id <= 0 || now < 0 {
        return std::ptr::null_mut();
    }
    let Some(bytes) = direct(env, bytes, crate::route_import::MAX_BYTES) else {
        return std::ptr::null_mut();
    };
    let task =
        match NavigationTask::prepare("selected-route.json", bytes, scope(first, last), now as u64)
        {
            Ok(task) => task,
            Err(error) => return output(env, &serde_json_lenient::json!({"error":error})),
        };
    let value = payload(&task, NavigationCommand::Observe);
    let Ok(mut slot) = TASK.lock() else {
        return std::ptr::null_mut();
    };
    if slot.as_ref().is_some_and(|(current, _)| *current > id) {
        return std::ptr::null_mut();
    }
    *slot = Some((id, task));
    output(env, &value)
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_NavigationNative_update(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    now: i64,
    condition: i32,
    captured_at: i64,
    buffer: *mut c_void,
) -> *mut c_void {
    if now < 0 || captured_at < 0 {
        return std::ptr::null_mut();
    }
    let Ok(mut slot) = TASK.lock() else {
        return std::ptr::null_mut();
    };
    let Some((current, task)) = slot.as_mut() else {
        return std::ptr::null_mut();
    };
    if *current != id {
        return std::ptr::null_mut();
    }
    let mut condition = match condition {
        0 => Conditions::Ready,
        1 => Conditions::UserStopped,
        2 => Conditions::PermissionLost,
        3 => Conditions::ScreenLost,
        4 => Conditions::WrongForeground,
        5 => Conditions::DisplayChanged,
        6 => Conditions::Locked,
        _ => Conditions::PermissionLost,
    };
    let frame = if buffer.is_null() {
        None
    } else {
        match direct(env, buffer, MINI_SIZE * MINI_SIZE * 3) {
            Some(rgb) if rgb.len() == MINI_SIZE * MINI_SIZE * 3 => Some(MiniMapFrame {
                captured_at: captured_at as u64,
                rgb,
            }),
            _ => {
                condition = Conditions::ScreenLost;
                None
            }
        }
    };
    let command = task.step(now as u64, condition, frame);
    output(env, &payload(task, command))
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_NavigationNative_receipt(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    serial: i64,
    now: i64,
    success: u8,
    released: u8,
) -> *mut c_void {
    if now < 0 || serial <= 0 {
        return std::ptr::null_mut();
    }
    let Ok(mut slot) = TASK.lock() else {
        return std::ptr::null_mut();
    };
    let Some((current, task)) = slot.as_mut() else {
        return std::ptr::null_mut();
    };
    if *current != id {
        return std::ptr::null_mut();
    }
    task.input_finished(serial as u64, now as u64, success != 0, released != 0);
    let ended = matches!(
        task.report().state,
        NavigationState::Completed | NavigationState::Stopped | NavigationState::Failed
    );
    output(
        env,
        &payload(
            task,
            if ended {
                NavigationCommand::End
            } else {
                NavigationCommand::Observe
            },
        ),
    )
}
