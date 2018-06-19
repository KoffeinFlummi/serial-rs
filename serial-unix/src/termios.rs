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
use libc;

use std::mem;

use std::os::unix::prelude::RawFd;

#[allow(non_camel_case_types)]
pub type termios = libc::termios;

#[derive(PartialEq, Eq)]
pub enum Speed {
    Standard(libc::speed_t),
    Custom(libc::speed_t),
}

pub fn read(fd: RawFd) -> core::Result<termios> {
    let mut termios: termios = unsafe { mem::uninitialized() };

    unsafe {
        if libc::tcgetattr(fd, &mut termios) < 0 {
            return Err(super::error::last_os_error());
        }
    }

    Ok(termios)
}

pub fn write(fd: RawFd, termios: &termios) -> core::Result<()> {
    use libc::TCSANOW;

    unsafe {
        if libc::tcsetattr(fd, TCSANOW, termios) < 0 {
            return Err(super::error::last_os_error());
        }
    }

    Ok(())
}

pub fn flush(fd: RawFd) -> core::Result<()> {
    use libc::TCIOFLUSH;

    unsafe {
        if libc::tcflush(fd, TCIOFLUSH) < 0 {
            return Err(super::error::last_os_error());
        }
    }

    Ok(())
}

pub fn get_speed(termios: &termios) -> (Speed, Speed) {
    unsafe {
        let ospeed = Speed::Standard(libc::cfgetospeed(termios));
        let ispeed = Speed::Standard(libc::cfgetispeed(termios));

        (ospeed, ispeed)
    }
}

pub fn set_speed(termios: &mut termios, speed: Speed) -> core::Result<()> {
    use libc::EINVAL;

    match speed {
        Speed::Standard(baud) => unsafe {
            if libc::cfsetspeed(termios, baud) < 0 {
                return Err(super::error::last_os_error());
            }
        },
        Speed::Custom(_) => {
            return Err(super::error::from_raw_os_error(EINVAL));
        }
    }

    Ok(())
}
