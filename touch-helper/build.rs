use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=native/device.c");
    println!("cargo:rerun-if-env-changed=PAIMON_NDK");
    if env::var("CARGO_CFG_TARGET_OS").unwrap() != "android" {
        return;
    }
    assert_eq!(
        env::var("TARGET").unwrap(),
        "aarch64-linux-android",
        "only arm64 Android is supported"
    );
    let ndk =
        PathBuf::from(env::var_os("PAIMON_NDK").expect("set PAIMON_NDK to NDK 27.0.12077973"));
    let host = if cfg!(windows) {
        "windows-x86_64"
    } else if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else {
        "linux-x86_64"
    };
    let toolchain = ndk.join("toolchains/llvm/prebuilt").join(host);
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let clang = toolchain.join(format!("bin/clang{extension}"));
    let ar = toolchain.join(format!("bin/llvm-ar{extension}"));
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = output.join("device.o");
    let status = Command::new(clang)
        .args([
            "--target=aarch64-linux-android34",
            "-std=c11",
            "-O2",
            "-fPIC",
            "-Wall",
            "-Wextra",
            "-Werror",
        ])
        .arg(format!("--sysroot={}", toolchain.join("sysroot").display()))
        .args(["-c", "native/device.c", "-o"])
        .arg(&object)
        .status()
        .expect("run NDK clang");
    assert!(status.success(), "native device adapter compilation failed");
    assert!(Command::new(ar)
        .arg("rcs")
        .arg(output.join("libtouch_device.a"))
        .arg(object)
        .status()
        .expect("run NDK ar")
        .success());
    println!("cargo:rustc-link-search=native={}", output.display());
    println!("cargo:rustc-link-lib=static=touch_device");
}
