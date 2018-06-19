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

#![allow(non_camel_case_types,dead_code)]

use libc;

use std::io;
use std::time::Duration;

use libc::{c_int, c_short};

#[cfg(target_os = "linux")]
type nfds_t = libc::c_ulong;

#[cfg(not(target_os = "linux"))]
type nfds_t = libc::c_uint;

#[derive(Debug)]
#[repr(C)]
struct pollfd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

const POLLIN:   c_short = 0x0001;
const POLLPRI:  c_short = 0x0002;
const POLLOUT:  c_short = 0x0004;

const POLLERR:  c_short = 0x0008;
const POLLHUP:  c_short = 0x0010;
const POLLNVAL: c_short = 0x0020;

pub fn wait_read_fd(fd: c_int, timeout: Duration) -> io::Result<()> {
    wait_fd(fd, POLLIN, timeout)
}

pub fn wait_write_fd(fd: c_int, timeout: Duration) -> io::Result<()> {
    wait_fd(fd, POLLOUT, timeout)
}

fn wait_fd(fd: c_int, events: c_short, timeout: Duration) -> io::Result<()> {
    use libc::{EINTR, EPIPE, EIO};

    let mut pollfd = pollfd {
        fd: fd,
        events: events,
        revents: 0,
    };

    let wait = do_poll(&mut pollfd, timeout);

    if wait < 0 {
        let errno = super::error::errno();

        let kind = match errno {
            EINTR => io::ErrorKind::Interrupted,
            _ => io::ErrorKind::Other,
        };

        return Err(io::Error::new(kind, super::error::error_string(errno)));
    }

    if wait == 0 {
        return Err(io::Error::new(io::ErrorKind::TimedOut, "Operation timed out"));
    }

    if pollfd.revents & events != 0 {
        return Ok(());
    }

    if pollfd.revents & (POLLHUP | POLLNVAL) != 0 {
        return Err(io::Error::new(io::ErrorKind::BrokenPipe, super::error::error_string(EPIPE)));
    }

    Err(io::Error::new(io::ErrorKind::Other, super::error::error_string(EIO)))
}

#[cfg(target_os = "linux")]
#[inline]
fn do_poll(pollfd: &mut pollfd, timeout: Duration) -> c_int {
    use std::ptr;

    use libc::c_void;

    #[repr(C)]
    struct sigset_t {
        __private: c_void,
    }

    extern "C" {
        fn ppoll(fds: *mut pollfd, nfds: nfds_t, timeout_ts: *mut libc::timespec, sigmask: *const sigset_t) -> c_int;
    }

    let mut timeout_ts = libc::timespec {
        tv_sec: timeout.as_secs() as libc::time_t,
        tv_nsec: timeout.subsec_nanos() as libc::c_long,
    };

    unsafe {
        ppoll(pollfd, 1, &mut timeout_ts, ptr::null())
    }
}

#[cfg(not(target_os = "linux"))]
#[inline]
fn do_poll(pollfd: &mut pollfd, timeout: Duration) -> c_int {
    extern "C" {
        fn poll(fds: *mut pollfd, nfds: nfds_t, timeout: c_int) -> c_int;
    }

    let milliseconds = timeout.as_secs() * 1000 + timeout.subsec_nanos() as u64 / 1_000_000;

    unsafe {
        poll(pollfd, 1, milliseconds as c_int)
    }
}
