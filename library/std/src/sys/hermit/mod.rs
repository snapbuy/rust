//! System bindings for HermitCore
//!
//! This module contains the facade (aka platform-specific) implementations of
//! OS level functionality for HermitCore.
//!
//! This is all super highly experimental and not actually intended for
//! wide/production use yet, it's still all in the experimental category. This
//! will likely change over time.
//!
//! Currently all functions here are basically stubs that immediately return
//! errors. The hope is that with a portability lint we can turn actually just
//! remove all this and just omit parts of the standard library if we're
//! compiling for wasm. That way it's a compile time error for something that's
//! guaranteed to be a runtime error!

#![allow(unsafe_op_in_unsafe_fn)]

use crate::intrinsics;
use crate::os::raw::c_char;

pub mod alloc;
pub mod args;
#[path = "../unix/cmath.rs"]
pub mod cmath;
pub mod condvar;
pub mod env;
pub mod fd;
pub mod fs;
#[path = "../unsupported/io.rs"]
pub mod io;
pub mod memchr;
pub mod mutex;
pub mod net;
pub mod os;
#[path = "../unix/path.rs"]
pub mod path;
#[path = "../unsupported/pipe.rs"]
pub mod pipe;
#[path = "../unsupported/process.rs"]
pub mod process;
pub mod rwlock;
pub mod stdio;
pub mod thread;
pub mod thread_local_dtor;
#[path = "../unsupported/thread_local_key.rs"]
pub mod thread_local_key;
pub mod time;

use crate::io::ErrorKind;
pub use crate::sys_common::os_str_bytes as os_str;

#[allow(unused_extern_crates)]
pub extern crate hermit_abi as abi;

pub fn unsupported<T>() -> crate::io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> crate::io::Error {
    crate::io::Error::new_const(
        crate::io::ErrorKind::Unsupported,
        &"operation not supported on HermitCore yet",
    )
}

pub unsafe fn strlen(start: *const c_char) -> usize {
    let mut str = start;

    while *str != 0 {
        str = str.offset(1);
    }

    (str as usize) - (start as usize)
}

#[no_mangle]
pub extern "C" fn floor(x: f64) -> f64 {
    unsafe { intrinsics::floorf64(x) }
}

pub fn abort_internal() -> ! {
    unsafe {
        abi::abort();
    }
}

// FIXME: just a workaround to test the system
pub fn hashmap_random_keys() -> (u64, u64) {
    (1, 2)
}

// This function is needed by the panic runtime. The symbol is named in
// pre-link args for the target specification, so keep that in sync.
#[cfg(not(test))]
#[no_mangle]
// NB. used by both libunwind and libpanic_abort
pub extern "C" fn __rust_abort() {
    abort_internal();
}

// SAFETY: must be called only once during runtime initialization.
// NOTE: this is not guaranteed to run, for example when Rust code is called externally.
pub unsafe fn init(argc: isize, argv: *const *const u8) {
    let _ = net::init();
    args::init(argc, argv);
}

// SAFETY: must be called only once during runtime cleanup.
// NOTE: this is not guaranteed to run, for example when the program aborts.
pub unsafe fn cleanup() {
    args::cleanup();
}

#[cfg(not(test))]
#[no_mangle]
pub unsafe extern "C" fn runtime_entry(
    argc: i32,
    argv: *const *const c_char,
    env: *const *const c_char,
) -> ! {
    use crate::sys::hermit::thread_local_dtor::run_dtors;
    extern "C" {
        fn main(argc: isize, argv: *const *const c_char) -> i32;
    }

    // initialize environment
    os::init_environment(env as *const *const i8);

    let result = main(argc as isize, argv);

    run_dtors();
    abi::exit(result);
}

pub fn decode_error_kind(errno: i32) -> ErrorKind {
    match errno {
        x if x == 13 as i32 => ErrorKind::PermissionDenied,
        x if x == 98 as i32 => ErrorKind::AddrInUse,
        x if x == 99 as i32 => ErrorKind::AddrNotAvailable,
        x if x == 11 as i32 => ErrorKind::WouldBlock,
        x if x == 103 as i32 => ErrorKind::ConnectionAborted,
        x if x == 111 as i32 => ErrorKind::ConnectionRefused,
        x if x == 104 as i32 => ErrorKind::ConnectionReset,
        x if x == 17 as i32 => ErrorKind::AlreadyExists,
        x if x == 4 as i32 => ErrorKind::Interrupted,
        x if x == 22 as i32 => ErrorKind::InvalidInput,
        x if x == 2 as i32 => ErrorKind::NotFound,
        x if x == 107 as i32 => ErrorKind::NotConnected,
        x if x == 1 as i32 => ErrorKind::PermissionDenied,
        x if x == 32 as i32 => ErrorKind::BrokenPipe,
        x if x == 110 as i32 => ErrorKind::TimedOut,
        _ => ErrorKind::Other,
    }
}

pub fn cvt(result: i32) -> crate::io::Result<usize> {
    if result < 0 {
        Err(crate::io::Error::from_raw_os_error(-result))
    } else {
        Ok(result as usize)
    }
}
