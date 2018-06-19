// Copyright (c) 2015 David Cuddeback
//
// Permission is hereby granted, free of charge, to any person obtaining
// a copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to
// permit persons to whom the Software is furnished to do so, subject to
// the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE
// LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION
// WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use core;

use std::ffi::CStr;
use std::io;
use std::str;

use libc::{c_int, c_char, size_t};

pub fn last_os_error() -> core::Error {
    from_raw_os_error(errno())
}

pub fn from_raw_os_error(errno: i32) -> core::Error {
    use libc::{EBUSY, EISDIR, ELOOP, ENOTDIR, ENOENT, ENODEV, ENXIO, EACCES, EINVAL, ENAMETOOLONG, EINTR, EWOULDBLOCK};

    let kind = match errno {
        EBUSY | EISDIR | ELOOP | ENOTDIR | ENOENT | ENODEV | ENXIO | EACCES => core::ErrorKind::NoDevice,
        EINVAL | ENAMETOOLONG => core::ErrorKind::InvalidInput,

        EINTR       => core::ErrorKind::Io(io::ErrorKind::Interrupted),
        EWOULDBLOCK => core::ErrorKind::Io(io::ErrorKind::WouldBlock),
        _           => core::ErrorKind::Io(io::ErrorKind::Other),
    };

    core::Error::new(kind, error_string(errno))
}

// the rest of this module is borrowed from libstd

const TMPBUF_SZ: usize = 128;

pub fn errno() -> i32 {
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "freebsd"))]
    unsafe fn errno_location() -> *const c_int {
        extern { fn __error() -> *const c_int; }
        __error()
    }

    #[cfg(target_os = "bitrig")]
    fn errno_location() -> *const c_int {
        extern {
            fn __errno() -> *const c_int;
        }
        unsafe {
            __errno()
        }
    }

    #[cfg(target_os = "dragonfly")]
    unsafe fn errno_location() -> *const c_int {
        extern { fn __dfly_error() -> *const c_int; }
        __dfly_error()
    }

    #[cfg(target_os = "openbsd")]
    unsafe fn errno_location() -> *const c_int {
        extern { fn __errno() -> *const c_int; }
        __errno()
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    unsafe fn errno_location() -> *const c_int {
        extern { fn __errno_location() -> *const c_int; }
        __errno_location()
    }

    unsafe {
        (*errno_location()) as i32
    }
}

pub fn error_string(errno: i32) -> String {
    #[cfg(target_os = "linux")]
    extern {
        #[link_name = "__xpg_strerror_r"]
        fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    }
    #[cfg(not(target_os = "linux"))]
    extern {
        fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: size_t) -> c_int;
    }

    let mut buf = [0 as c_char; TMPBUF_SZ];

    let p = buf.as_mut_ptr();
    unsafe {
        if strerror_r(errno as c_int, p, buf.len() as size_t) < 0 {
            panic!("strerror_r failure");
        }

        let p = p as *const _;
        str::from_utf8(CStr::from_ptr(p).to_bytes()).unwrap().to_string()
    }
}
