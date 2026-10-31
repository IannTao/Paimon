use crate::{
    localization::{LocalizationTask, MiniMapFrame, MINI_SIZE},
    route_import::import_route,
};
use std::{ffi::c_void, sync::Mutex};

static TASK: Mutex<Option<(i64, LocalizationTask)>> = Mutex::new(None);

use crate::android_jni::{direct, output};

#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_LocalizationNative_start(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    route: *mut c_void,
) -> *mut c_void {
    let Some(bytes) = direct(env, route, crate::route_import::MAX_BYTES) else {
        return std::ptr::null_mut();
    };
    if id <= 0 {
        return std::ptr::null_mut();
    }
    let mut task = LocalizationTask::new(&import_route("selected-route.json", bytes));
    let report = task.snapshot(0);
    let Ok(mut slot) = TASK.lock() else {
        return std::ptr::null_mut();
    };
    if slot.as_ref().is_some_and(|(current, _)| *current > id) {
        return std::ptr::null_mut();
    }
    *slot = Some((id, task));
    output(env, &report)
}
#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_LocalizationNative_update(
    env: *mut c_void,
    _: *mut c_void,
    id: i64,
    now: i64,
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
    let report = match direct(env, buffer, MINI_SIZE * MINI_SIZE * 3) {
        Some(rgb) => task.observe(
            now as u64,
            MiniMapFrame {
                captured_at: captured_at as u64,
                rgb,
            },
        ),
        None => task.invalidate(),
    };
    output(env, &report)
}
