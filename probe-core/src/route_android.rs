//! Synchronous JNI adapter. Both direct buffers remain owned by the caller.
use std::ffi::c_void;

#[no_mangle]
pub unsafe extern "system" fn Java_dev_paimon_probe_RouteNative_inspect(
    env: *mut c_void,
    _: *mut c_void,
    data: *mut c_void,
    name: *mut c_void,
) -> *mut c_void {
    let functions = *(env as *const *const *const c_void);
    let address: unsafe extern "system" fn(*mut c_void, *mut c_void) -> *const u8 =
        std::mem::transmute(*functions.add(230));
    let capacity: unsafe extern "system" fn(*mut c_void, *mut c_void) -> i64 =
        std::mem::transmute(*functions.add(231));
    let new_array: unsafe extern "system" fn(*mut c_void, i32) -> *mut c_void =
        std::mem::transmute(*functions.add(176));
    let set_region: unsafe extern "system" fn(*mut c_void, *mut c_void, i32, i32, *const u8) =
        std::mem::transmute(*functions.add(208));
    if data.is_null() || name.is_null() {
        return std::ptr::null_mut();
    }
    let len = capacity(env, data);
    let name_len = capacity(env, name);
    if !(0..=crate::route_import::MAX_BYTES as i64).contains(&len)
        || !(1..=4096).contains(&name_len)
    {
        return std::ptr::null_mut();
    }
    let data_ptr = address(env, data);
    let name_ptr = address(env, name);
    if data_ptr.is_null() || name_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(name) = std::str::from_utf8(std::slice::from_raw_parts(name_ptr, name_len as usize))
    else {
        return std::ptr::null_mut();
    };
    let report =
        crate::route_import::import_route(name, std::slice::from_raw_parts(data_ptr, len as usize));
    let Ok(json) = serde_json_lenient::to_vec(&report) else {
        return std::ptr::null_mut();
    };
    let Ok(size) = i32::try_from(json.len()) else {
        return std::ptr::null_mut();
    };
    let result = new_array(env, size);
    if !result.is_null() {
        set_region(env, result, 0, size, json.as_ptr());
    }
    result
}
