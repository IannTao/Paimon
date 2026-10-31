use crate::{
    localization::{LocalizationTask, MiniMapFrame, MINI_SIZE},
    route_import::import_route,
};
use std::{ffi::c_void, sync::Mutex};

static TASK: Mutex<Option<(i64, LocalizationTask)>> = Mutex::new(None);

unsafe fn direct<'a>(env: *mut c_void, buffer: *mut c_void, max: usize) -> Option<&'a [u8]> {
    if buffer.is_null() {
        return None;
    }
    let functions = *(env as *const *const *const c_void);
    let address: unsafe extern "system" fn(*mut c_void, *mut c_void) -> *const u8 =
        std::mem::transmute(*functions.add(230));
    let capacity: unsafe extern "system" fn(*mut c_void, *mut c_void) -> i64 =
        std::mem::transmute(*functions.add(231));
    let len = capacity(env, buffer);
    if !(0..=max as i64).contains(&len) {
        return None;
    }
    let pointer = address(env, buffer);
    if pointer.is_null() {
        return None;
    }
    Some(std::slice::from_raw_parts(pointer, len as usize))
}
unsafe fn output(env: *mut c_void, value: &impl serde::Serialize) -> *mut c_void {
    let Ok(bytes) = serde_json_lenient::to_vec(value) else {
        return std::ptr::null_mut();
    };
    let Ok(len) = i32::try_from(bytes.len()) else {
        return std::ptr::null_mut();
    };
    let functions = *(env as *const *const *const c_void);
    let new: unsafe extern "system" fn(*mut c_void, i32) -> *mut c_void =
        std::mem::transmute(*functions.add(176));
    let set: unsafe extern "system" fn(*mut c_void, *mut c_void, i32, i32, *const u8) =
        std::mem::transmute(*functions.add(208));
    let array = new(env, len);
    if !array.is_null() {
        set(env, array, 0, len, bytes.as_ptr());
    }
    array
}
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
