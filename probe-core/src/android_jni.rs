// Synchronous JNI buffers remain owned by the caller; no references escape the call.
use std::ffi::c_void;
pub(crate) unsafe fn direct<'a>(
    env: *mut c_void,
    buffer: *mut c_void,
    max: usize,
) -> Option<&'a [u8]> {
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
pub(crate) unsafe fn output(env: *mut c_void, value: &impl serde::Serialize) -> *mut c_void {
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
