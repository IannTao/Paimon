use crate::{
    android_jni::{direct, output},
    localization::MINI_SIZE,
    movement::Conditions,
    route_execution::{ExecutionScope, RouteCommand, RouteFrame, RouteTask},
};
use std::{ffi::c_void, sync::Mutex};
static TASK: Mutex<Option<(i64, RouteTask)>> = Mutex::new(None);
fn scope(code: i32) -> Result<ExecutionScope, String> {
    // 与Kotlin适配层约定的范围码；未知值拒绝，不能意外退回整条路线执行。
    match code {
        0 => Ok(ExecutionScope::WholeRoute),
        1 => Ok(ExecutionScope::FirstTeleport),
        2 => Ok(ExecutionScope::FirstPickup),
        _ => Err("未知路线执行范围".into()),
    }
}
fn payload(task: &RouteTask, command: RouteCommand) -> serde_json_lenient::Value {
    serde_json_lenient::json!({"task":task.report(),"command":command})
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_RouteExecutionNative_plan(
    env: *mut c_void,
    _: *mut c_void,
    bytes: *mut c_void,
    scope_code: i32,
) -> *mut c_void {
    let Some(bytes) = direct(env, bytes, crate::route_import::MAX_BYTES) else {
        return std::ptr::null_mut();
    };
    match scope(scope_code).and_then(|s| RouteTask::prepare("selected-route.json", bytes, s, 0)) {
        Ok(task) => output(env, &payload(&task, RouteCommand::Observe)),
        Err(error) => output(env, &serde_json_lenient::json!({"error":error})),
    }
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_RouteExecutionNative_start(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    bytes: *mut c_void,
    scope_code: i32,
    now: i64,
) -> *mut c_void {
    if id <= 0 || now < 0 {
        return std::ptr::null_mut();
    }
    let Some(bytes) = direct(env, bytes, crate::route_import::MAX_BYTES) else {
        return std::ptr::null_mut();
    };
    let task = match scope(scope_code)
        .and_then(|s| RouteTask::prepare("selected-route.json", bytes, s, now as u64))
    {
        Ok(t) => t,
        Err(error) => return output(env, &serde_json_lenient::json!({"error":error})),
    };
    let Ok(mut slot) = TASK.lock() else {
        return std::ptr::null_mut();
    };
    if slot.as_ref().is_some_and(|(current, _)| *current >= id) {
        return std::ptr::null_mut();
    }
    let result = payload(&task, RouteCommand::Observe);
    *slot = Some((id, task));
    output(env, &result)
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_RouteExecutionNative_update(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    now: i64,
    condition: i32,
    captured_at: i64,
    mini: *mut c_void,
    screen: *mut c_void,
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
        _ => Conditions::ScreenLost,
    };
    let minimap = if mini.is_null() {
        None
    } else {
        direct(env, mini, MINI_SIZE * MINI_SIZE * 3)
            .filter(|v| v.len() == MINI_SIZE * MINI_SIZE * 3)
    };
    let overview = if screen.is_null() {
        None
    } else {
        direct(env, screen, 1240 * 558 * 3).filter(|v| v.len() == 1240 * 558 * 3)
    };
    if (!mini.is_null() && minimap.is_none()) || (!screen.is_null() && overview.is_none()) {
        condition = Conditions::ScreenLost;
    }
    let frame = minimap.map(|minimap| RouteFrame {
        captured_at: captured_at as u64,
        screen: overview,
        minimap,
    });
    let command = task.step(now as u64, condition, frame);
    output(env, &payload(task, command))
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_RouteExecutionNative_receipt(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    serial: i64,
    now: i64,
    success: u8,
    released: u8,
) -> *mut c_void {
    if serial <= 0 || now < 0 {
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
    let command = if task.terminal() {
        RouteCommand::End
    } else {
        RouteCommand::Observe
    };
    output(env, &payload(task, command))
}
