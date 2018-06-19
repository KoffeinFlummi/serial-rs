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
pub type termios = libc::termios2;

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum Speed {
    Standard(libc::speed_t),
    Custom(libc::speed_t),
}

const IBSHIFT: usize = 16;
const BOTHER: libc::speed_t = 0x1000;

#[cfg(not(any(target_env = "musl",
              target_env = "android")))]
#[allow(non_camel_case_types)]
type ioctl_request = libc::c_ulong;

#[cfg(any(target_env = "musl",
          target_env = "android"))]
#[allow(non_camel_case_types)]
type ioctl_request = libc::c_int;

#[cfg(any(target_arch = "x86",
          target_arch = "x86_64",
          target_arch = "arm",
          target_arch = "aarch64",
          target_arch = "s390x"))]
// Suppress warning on targets with libc that use c_int for ioctl request type. Binary
// representation is unaffected, so ioctl() will be interpreted correctly.
#[allow(overflowing_literals)]
const TCGETS2: ioctl_request = 0x802C542A;

#[cfg(any(target_arch = "x86",
          target_arch = "x86_64",
          target_arch = "arm",
          target_arch = "aarch64",
          target_arch = "s390x"))]
const TCSETS2: ioctl_request = 0x402C542B;

#[cfg(any(target_arch = "mips",
          target_arch = "mips64",
          target_arch = "powerpc",
          target_arch = "powerpc64",
          target_arch = "sparc64"))]
const TCGETS2: ioctl_request = 0x402C542A;

#[cfg(any(target_arch = "mips",
          target_arch = "mips64",
          target_arch = "powerpc",
          target_arch = "powerpc64",
          target_arch = "sparc64"))]
// Suppress warning on targets with libc that use c_int for ioctl request type. Binary
// representation is unaffected, so ioctl() will be interpreted correctly.
#[allow(overflowing_literals)]
const TCSETS2: ioctl_request = 0x802C542B;

pub fn read(fd: RawFd) -> core::Result<termios> {
    let mut termios: termios = unsafe { mem::uninitialized() };

    unsafe {
        if libc::ioctl(fd, TCGETS2, &mut termios) < 0 {
            return Err(super::error::last_os_error());
        }
    }

    Ok(termios)
}

pub fn write(fd: RawFd, termios: &termios) -> core::Result<()> {
    unsafe {
        if libc::ioctl(fd, TCSETS2, termios) < 0 {
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

// See tty_termios_baud_rate() and tty_termios_input_baud_rate() in drivers/tty/tty_baudrate.c in
// the Linux kernel source.
pub fn get_speed(termios: &termios) -> (Speed, Speed) {
    use libc::{CBAUD, B0};

    let ospeed = match termios.c_cflag & CBAUD {
        BOTHER => Speed::Custom(termios.c_ospeed),
        speed => Speed::Standard(speed),
    };

    let ispeed = match termios.c_cflag >> IBSHIFT & CBAUD {
        B0 => ospeed,
        BOTHER => Speed::Custom(termios.c_ispeed),
        speed => Speed::Standard(speed),
    };

    (ospeed, ispeed)
}

// See tty_termios_baud_rate() and tty_termios_input_baud_rate() in drivers/tty/tty_baudrate.c in
// the Linux kernel source.
pub fn set_speed(termios: &mut termios, speed: Speed) -> core::Result<()> {
    use libc::{CBAUD, B0};

    termios.c_cflag &= !(CBAUD | CBAUD << IBSHIFT);
    termios.c_cflag |= B0 << IBSHIFT;

    match speed {
        Speed::Standard(baud) => {
            termios.c_cflag |= baud;
        },
        Speed::Custom(baud) => {
            termios.c_cflag |= BOTHER;
            termios.c_ospeed = baud;
        },
    }

    Ok(())
}
