//! Keeping the protocol channel to ourselves (ADR-0083).
//!
//! The worker's stdout carries the NDJSON protocol, and the plugins it loads write to
//! stdout too: Hadron prints its Csound function tables there, ending on a fragment
//! with no newline that glued itself to the worker's next line and lost it
//! (`plugin-stdout-desyncs-scan`). Asking plugins not to print is not an option, so
//! [`claim_stdout`] gives the protocol a private copy of the pipe and points standard
//! output, at every level a plugin might reach it, at stderr instead.
//!
//! "Every level" is the point. A plugin can write to stdout through:
//!
//! - the Win32 standard handle, `GetStdHandle(STD_OUTPUT_HANDLE)`, which is also what a
//!   statically linked C runtime reads when its DLL loads;
//! - the shared Universal C Runtime (`ucrtbase.dll`), whose file descriptor 1 was set
//!   up when this process started, before we could change anything;
//! - the legacy `msvcrt.dll` that MinGW builds use, which has a descriptor table of its
//!   own.
//!
//! Each needs redirecting separately, and all of it has to happen before the first
//! plugin loads.

use std::fs::File;

/// Take the protocol channel for ourselves and send everyone else's stdout to stderr.
///
/// Returns the real stdout, the only handle the protocol may be written through from
/// here on. `std::io::stdout()` follows the standard handle, so after this it writes to
/// stderr like everything else. Call once, before loading any plugin.
pub fn claim_stdout() -> std::io::Result<File> {
    platform::claim_stdout()
}

#[cfg(windows)]
mod platform {
    use std::ffi::c_int;
    use std::fs::File;
    use std::os::windows::io::FromRawHandle;

    use windows_sys::Win32::Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE};
    use windows_sys::Win32::System::Console::{
        GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
    };
    use windows_sys::Win32::System::Threading::GetCurrentProcess;

    // The Universal C Runtime the worker itself is linked against, which plugins built
    // with any recent Visual Studio share.
    unsafe extern "C" {
        fn _dup2(from: c_int, to: c_int) -> c_int;
    }

    pub fn claim_stdout() -> std::io::Result<File> {
        unsafe {
            let stdout = GetStdHandle(STD_OUTPUT_HANDLE);
            let stderr = GetStdHandle(STD_ERROR_HANDLE);

            // A copy of the pipe that nobody else knows about. The original handle
            // stays open behind the C runtimes' descriptor 1 until they are redirected
            // below, which closes it; the copy keeps the pipe alive.
            let process = GetCurrentProcess();
            let mut private: HANDLE = std::ptr::null_mut();
            if DuplicateHandle(process, stdout, process, &mut private, 0, 0, DUPLICATE_SAME_ACCESS)
                == 0
            {
                return Err(std::io::Error::last_os_error());
            }

            if SetStdHandle(STD_OUTPUT_HANDLE, stderr) == 0 {
                return Err(std::io::Error::last_os_error());
            }
            if _dup2(2, 1) != 0 {
                crate::diag!("could not redirect the C runtime's stdout (ucrtbase _dup2 failed)");
            }
            redirect_msvcrt();

            Ok(File::from_raw_handle(private))
        }
    }

    /// The same for the legacy `msvcrt.dll`, which MinGW-built plugins use.
    ///
    /// If it is not loaded yet, loading it now makes it read the standard handles,
    /// which already point at stderr; if it is, `_dup2` fixes its descriptor 1. It is
    /// left loaded either way, so the redirect cannot be undone by an unload and reload.
    fn redirect_msvcrt() {
        let lib = match unsafe { libloading::Library::new("msvcrt.dll") } {
            Ok(lib) => lib,
            Err(e) => {
                crate::diag!("msvcrt.dll not redirected: {}", e);
                return;
            }
        };
        match unsafe { lib.get::<unsafe extern "C" fn(c_int, c_int) -> c_int>(b"_dup2") } {
            Ok(dup2) => {
                if unsafe { dup2(2, 1) } != 0 {
                    crate::diag!("could not redirect msvcrt.dll's stdout (_dup2 failed)");
                }
            }
            Err(e) => crate::diag!("msvcrt.dll has no _dup2: {}", e),
        }
        std::mem::forget(lib);
    }
}

#[cfg(unix)]
mod platform {
    use std::ffi::c_int;
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    unsafe extern "C" {
        fn dup(fd: c_int) -> c_int;
        fn dup2(from: c_int, to: c_int) -> c_int;
    }

    /// One C library and one descriptor table, so one `dup2` covers every plugin.
    pub fn claim_stdout() -> std::io::Result<File> {
        unsafe {
            let private = dup(1);
            if private < 0 {
                return Err(std::io::Error::last_os_error());
            }
            if dup2(2, 1) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(File::from_raw_fd(private))
        }
    }
}
