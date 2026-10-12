//! `stdio::claim_stdout` keeps the protocol channel to the worker (ADR-0083).
//!
//! What it guards against is a plugin printing, and a plugin can print through any of
//! several layers. The test runs this test binary again as a child that claims stdout,
//! prints through every layer a plugin might use, and writes one protocol line through
//! the claimed handle; the parent checks that only the protocol line reached stdout.

use std::io::Write;
use std::process::Command;

const CHILD: &str = "VST_META_STDOUT_REDIRECT_CHILD";
const PROTOCOL: &str = "PROTOCOL-LINE";

#[test]
fn only_the_protocol_reaches_stdout_once_it_is_claimed() {
    if std::env::var_os(CHILD).is_some() {
        child();
        return;
    }

    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "only_the_protocol_reaches_stdout_once_it_is_claimed", "--nocapture"])
        .env(CHILD, "1")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "child failed:\n{stderr}");

    assert!(stdout.contains(PROTOCOL), "the protocol line is missing from stdout:\n{stdout}");
    for layer in LAYERS {
        assert!(!stdout.contains(layer), "{layer} reached stdout:\n{stdout}");
        assert!(stderr.contains(layer), "{layer} did not reach stderr:\n{stderr}");
    }
}

/// What each layer prints, so the parent can tell them apart.
#[cfg(windows)]
const LAYERS: &[&str] = &["RUST-STD", "UCRT-PUTS", "MSVCRT-PUTS"];
#[cfg(not(windows))]
const LAYERS: &[&str] = &["RUST-STD", "LIBC-PUTS"];

fn child() {
    let mut protocol = vst_meta::stdio::claim_stdout().expect("claim stdout");

    // The Win32 standard handle (or fd 1 on Unix): Rust's stdout follows it.
    print!("RUST-STD\n");
    std::io::stdout().flush().unwrap();

    platform::print_through_c_runtimes();

    writeln!(protocol, "{PROTOCOL}").unwrap();
    protocol.flush().unwrap();
}

#[cfg(windows)]
mod platform {
    use std::ffi::{c_char, c_int};

    unsafe extern "C" {
        // The Universal C Runtime, which this binary is linked against.
        fn puts(s: *const c_char) -> c_int;
        fn fflush(stream: *mut std::ffi::c_void) -> c_int;
    }

    pub fn print_through_c_runtimes() {
        unsafe {
            puts(c"UCRT-PUTS".as_ptr());
            fflush(std::ptr::null_mut());
        }

        // The legacy msvcrt.dll, as a MinGW-built plugin would use it.
        let lib = unsafe { libloading::Library::new("msvcrt.dll") }.expect("msvcrt.dll");
        unsafe {
            let puts = lib
                .get::<unsafe extern "C" fn(*const c_char) -> c_int>(b"puts")
                .unwrap();
            let fflush = lib
                .get::<unsafe extern "C" fn(*mut std::ffi::c_void) -> c_int>(b"fflush")
                .unwrap();
            puts(c"MSVCRT-PUTS".as_ptr());
            fflush(std::ptr::null_mut());
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::ffi::{c_char, c_int};

    unsafe extern "C" {
        fn puts(s: *const c_char) -> c_int;
        fn fflush(stream: *mut std::ffi::c_void) -> c_int;
    }

    pub fn print_through_c_runtimes() {
        unsafe {
            puts(c"LIBC-PUTS".as_ptr());
            fflush(std::ptr::null_mut());
        }
    }
}
