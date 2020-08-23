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

use std::ffi::CString;
use std::fmt;
use std::io;
use std::mem;
use std::path::Path;
use std::time::Duration;

use std::os::unix::prelude::*;

use libc::{c_int, c_void, size_t};

use core::{SerialDevice, SerialPortSettings};


#[cfg(target_os = "linux")]
const IBSHIFT: usize = 16;

#[cfg(all(target_os = "linux",
          not(any(target_env = "musl",
                  target_env = "android"))))]
#[allow(non_camel_case_types)]
type ioctl_request = libc::c_ulong;

#[cfg(all(target_os = "linux",
          any(target_env = "musl",
              target_env = "android")))]
#[allow(non_camel_case_types)]
type ioctl_request = libc::c_int;

#[cfg(all(target_os = "linux",
          any(target_arch = "x86",
              target_arch = "x86_64",
              target_arch = "arm",
              target_arch = "aarch64",
              target_arch = "s390x")))]
// Suppress warning on targets with libc that use c_int for ioctl request type. Binary
// representation is unaffected, so ioctl() will be interpreted correctly.
#[allow(overflowing_literals)]
const TCGETS2: ioctl_request = 0x802C542A;

#[cfg(all(target_os = "linux",
          any(target_arch = "x86",
              target_arch = "x86_64",
              target_arch = "arm",
              target_arch = "aarch64",
              target_arch = "s390x")))]
const TCSETS2: ioctl_request = 0x402C542B;

#[cfg(all(target_os = "linux",
          any(target_arch = "mips",
              target_arch = "mips64",
              target_arch = "powerpc",
              target_arch = "powerpc64",
              target_arch = "sparc64")))]
const TCGETS2: ioctl_request = 0x402C542A;

#[cfg(all(target_os = "linux",
          any(target_arch = "mips",
              target_arch = "mips64",
              target_arch = "powerpc",
              target_arch = "powerpc64",
              target_arch = "sparc64")))]
// Suppress warning on targets with libc that use c_int for ioctl request type. Binary
// representation is unaffected, so ioctl() will be interpreted correctly.
#[allow(overflowing_literals)]
const TCSETS2: ioctl_request = 0x802C542B;


/// A TTY-based serial port implementation.
///
/// The port will be closed when the value is dropped.
pub struct TTYPort {
    fd: RawFd,
    timeout: Duration,
}

impl TTYPort {
    /// Opens a TTY device as a serial port.
    ///
    /// `path` should be the path to a TTY device, e.g., `/dev/ttyS0`.
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// serial_unix::TTYPort::open(Path::new("/dev/ttyS0")).unwrap();
    /// ```
    ///
    /// ## Errors
    ///
    /// * `NoDevice` if the device could not be opened. This could indicate that the device is
    ///   already in use.
    /// * `InvalidInput` if `port` is not a valid device name.
    /// * `Io` for any other error while opening or initializing the device.
    pub fn open(path: &Path) -> core::Result<Self> {
        use libc::{O_RDWR, O_NOCTTY, O_NONBLOCK, TIOCEXCL, F_SETFL, EINVAL};

        let cstr = match CString::new(path.as_os_str().as_bytes()) {
            Ok(s) => s,
            Err(_) => return Err(super::error::from_raw_os_error(EINVAL)),
        };

        let fd = unsafe { libc::open(cstr.as_ptr(), O_RDWR | O_NOCTTY | O_NONBLOCK, 0) };
        if fd < 0 {
            return Err(super::error::last_os_error());
        }

        let mut port = TTYPort {
            fd: fd,
            timeout: Duration::from_millis(100),
        };

        unsafe {
            // get exclusive access to device
            if libc::ioctl(port.fd, TIOCEXCL) < 0 {
                return Err(super::error::last_os_error());
            }

            // clear O_NONBLOCK flag
            if libc::fcntl(port.fd, F_SETFL, 0) < 0 {
                return Err(super::error::last_os_error());
            }
        }

        // apply initial settings
        let settings = try!(port.read_settings());
        try!(port.write_settings(&settings));

        Ok(port)
    }

    fn set_pin(&mut self, pin: c_int, level: bool) -> core::Result<()> {
        use libc::{TIOCMBIS, TIOCMBIC};

        let retval = unsafe {
            if level {
                libc::ioctl(self.fd, TIOCMBIS, &pin)
            }
            else {
                libc::ioctl(self.fd, TIOCMBIC, &pin)
            }
        };

        if retval < 0 {
            return Err(super::error::last_os_error());
        }

        Ok(())
    }

    fn read_pin(&mut self, pin: c_int) -> core::Result<bool> {
        use libc::{TIOCMGET};

        unsafe {
            let mut pins: c_int = mem::uninitialized();

            if libc::ioctl(self.fd, TIOCMGET, &mut pins) < 0 {
                return Err(super::error::last_os_error());
            }

            Ok(pins & pin != 0)
        }
    }
}

impl Drop for TTYPort {
    fn drop(&mut self) {
        use libc::{TIOCNXCL};

        unsafe {
            libc::ioctl(self.fd, TIOCNXCL);
            libc::close(self.fd);
        }
    }
}

impl AsRawFd for TTYPort {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl io::Read for TTYPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        try!(super::poll::wait_read_fd(self.fd, self.timeout));

        let len = unsafe {
            libc::read(self.fd, buf.as_ptr() as *mut c_void, buf.len() as size_t)
        };

        if len >= 0 {
            Ok(len as usize)
        }
        else {
            Err(io::Error::last_os_error())
        }
    }
}

impl io::Write for TTYPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        try!(super::poll::wait_write_fd(self.fd, self.timeout));

        let len = unsafe {
            libc::write(self.fd, buf.as_ptr() as *mut c_void, buf.len() as size_t)
        };

        if len >= 0 {
            Ok(len as usize)
        }
        else {
            Err(io::Error::last_os_error())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        unsafe {
            if libc::tcdrain(self.fd) < 0 {
                Err(io::Error::last_os_error())
            }
            else {
                Ok(())
            }
        }
    }
}

impl SerialDevice for TTYPort {
    type Settings = TTYSettings;

    fn read_settings(&self) -> core::Result<TTYSettings> {
        use libc::{CREAD, CLOCAL}; // cflags
        use libc::{ICANON, ECHO, ECHOE, ECHOK, ECHONL, ISIG, IEXTEN}; // lflags
        use libc::{OPOST}; // oflags
        use libc::{INLCR, IGNCR, ICRNL, IGNBRK}; // iflags
        use libc::{VMIN, VTIME}; // c_cc indexes

        #[cfg(not(target_os = "linux"))]
        let mut termios: libc::termios = unsafe { mem::uninitialized() };

        #[cfg(target_os = "linux")]
        let mut termios: libc::termios2 = unsafe { mem::uninitialized() };

        #[cfg(not(target_os = "linux"))]
        unsafe {
            if libc::tcgetattr(self.fd, &mut termios) < 0 {
                return Err(super::error::last_os_error());
            }
        }

        #[cfg(target_os = "linux")]
        unsafe {
            if libc::ioctl(self.fd, TCGETS2, &mut termios) < 0 {
                return Err(super::error::last_os_error());
            }
        }

        // setup TTY for binary serial port access
        termios.c_cflag |= CREAD | CLOCAL;
        termios.c_lflag &= !(ICANON | ECHO | ECHOE | ECHOK | ECHONL | ISIG | IEXTEN);
        termios.c_oflag &= !OPOST;
        termios.c_iflag &= !(INLCR | IGNCR | ICRNL | IGNBRK);

        termios.c_cc[VMIN] = 0;
        termios.c_cc[VTIME] = 0;

        Ok(TTYSettings::new(termios))
    }

    fn write_settings(&mut self, settings: &TTYSettings) -> core::Result<()> {
        #[cfg(not(target_os = "linux"))]
        use libc::{TCSANOW};
        use libc::{TCIOFLUSH};

        // write settings to TTY
        #[cfg(not(target_os = "linux"))]
        unsafe {
            if libc::tcsetattr(self.fd, TCSANOW, &settings.termios) < 0 {
                return Err(super::error::last_os_error());
            }
        }

        #[cfg(target_os = "linux")]
        unsafe {
            if libc::ioctl(self.fd, TCSETS2, &settings.termios) < 0 {
                return Err(super::error::last_os_error());
            }
        }

        unsafe {
            if libc::tcflush(self.fd, TCIOFLUSH) < 0 {
                return Err(super::error::last_os_error());
            }
        }

        Ok(())
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }

    fn set_timeout(&mut self, timeout: Duration) -> core::Result<()> {
        self.timeout = timeout;
        Ok(())
    }

    fn set_rts(&mut self, level: bool) -> core::Result<()> {
        self.set_pin(libc::TIOCM_RTS, level)
    }

    fn set_dtr(&mut self, level: bool) -> core::Result<()> {
        self.set_pin(libc::TIOCM_DTR, level)
    }

    fn read_cts(&mut self) -> core::Result<bool> {
        self.read_pin(libc::TIOCM_CTS)
    }

    fn read_dsr(&mut self) -> core::Result<bool> {
        self.read_pin(libc::TIOCM_DSR)
    }

    fn read_ri(&mut self) -> core::Result<bool> {
        self.read_pin(libc::TIOCM_RI)
    }

    fn read_cd(&mut self) -> core::Result<bool> {
        self.read_pin(libc::TIOCM_CD)
    }
}

/// Serial port settings for TTY devices.
#[derive(Copy,Clone)]
pub struct TTYSettings {
    #[cfg(not(target_os = "linux"))]
    termios: libc::termios,
    #[cfg(target_os = "linux")]
    termios: libc::termios2,
}

impl TTYSettings {
    #[cfg(not(target_os = "linux"))]
    fn new(termios: libc::termios) -> Self {
        TTYSettings { termios: termios }
    }

    #[cfg(target_os = "linux")]
    fn new(termios: libc::termios2) -> Self {
        TTYSettings { termios: termios }
    }
}

impl fmt::Debug for TTYSettings {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        #[cfg(not(target_os = "linux"))]
        struct TermiosFormatter(libc::termios);
        #[cfg(target_os = "linux")]
        struct TermiosFormatter(libc::termios2);

        impl fmt::Debug for TermiosFormatter {
            fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
                f.debug_struct("termios")
                    .field("c_iflag", &self.0.c_iflag)
                    .field("c_oflag", &self.0.c_oflag)
                    .field("c_cflag", &self.0.c_cflag)
                    .field("c_lflag", &self.0.c_lflag)
                    .field("c_cc", &self.0.c_cc)
                    .finish()
            }
        }

        f.debug_struct("TTYSettings")
            .field("termios", &TermiosFormatter(self.termios))
            .finish()
    }
}

impl SerialPortSettings for TTYSettings {
    fn baud_rate(&self) -> Option<core::BaudRate> {
        use libc::{B50, B75, B110, B134, B150, B200, B300, B600, B1200, B1800, B2400, B4800, B9600, B19200, B38400};
        use libc::{B57600, B115200, B230400};

        #[cfg(target_os = "linux")]
        use libc::{B0, B460800, B500000, B576000, B921600, B1000000, B1152000, B1500000, B2000000, B2500000, B3000000, B3500000, B4000000};

        #[cfg(target_os = "macos")]
        use libc::{B7200, B14400, B28800, B76800};

        #[cfg(target_os = "freebsd")]
        use libc::{B7200, B14400, B28800, B76800, B460800, B921600};

        #[cfg(target_os = "openbsd")]
        use libc::{B7200, B14400, B28800, B76800};

        #[cfg(target_os = "linux")]
        use libc::CBAUD;

        #[cfg(not(target_os = "linux"))]
        let (ospeed, ispeed) = unsafe {
            (
                libc::cfgetospeed(&self.termios),
                libc::cfgetispeed(&self.termios),
            )
        };

        #[cfg(target_os = "linux")]
        let (ospeed, ispeed) = {
            let ospeed = self.termios.c_cflag & CBAUD;
            let ispeed = self.termios.c_cflag >> IBSHIFT & CBAUD;

            match ispeed {
                B0 => (ospeed, ospeed),
                _ => (ospeed, ispeed),
            }
        };

        if ospeed != ispeed {
            return None;
        }

        match ospeed {
            B50      => Some(core::BaudOther(50)),
            B75      => Some(core::BaudOther(75)),
            B110     => Some(core::Baud110),
            B134     => Some(core::BaudOther(134)),
            B150     => Some(core::BaudOther(150)),
            B200     => Some(core::BaudOther(200)),
            B300     => Some(core::Baud300),
            B600     => Some(core::Baud600),
            B1200    => Some(core::Baud1200),
            B1800    => Some(core::BaudOther(1800)),
            B2400    => Some(core::Baud2400),
            B4800    => Some(core::Baud4800),
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            B7200    => Some(core::BaudOther(7200)),
            B9600    => Some(core::Baud9600),
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            B14400   => Some(core::BaudOther(14400)),
            B19200   => Some(core::Baud19200),
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            B28800   => Some(core::BaudOther(28800)),
            B38400   => Some(core::Baud38400),
            B57600   => Some(core::Baud57600),
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            B76800   => Some(core::BaudOther(76800)),
            B115200  => Some(core::Baud115200),
            B230400  => Some(core::BaudOther(230400)),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            B460800  => Some(core::BaudOther(460800)),
            #[cfg(target_os = "linux")]
            B500000  => Some(core::BaudOther(500000)),
            #[cfg(target_os = "linux")]
            B576000  => Some(core::BaudOther(576000)),
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            B921600  => Some(core::BaudOther(921600)),
            #[cfg(target_os = "linux")]
            B1000000 => Some(core::BaudOther(1000000)),
            #[cfg(target_os = "linux")]
            B1152000 => Some(core::BaudOther(1152000)),
            #[cfg(target_os = "linux")]
            B1500000 => Some(core::BaudOther(1500000)),
            #[cfg(target_os = "linux")]
            B2000000 => Some(core::BaudOther(2000000)),
            #[cfg(target_os = "linux")]
            B2500000 => Some(core::BaudOther(2500000)),
            #[cfg(target_os = "linux")]
            B3000000 => Some(core::BaudOther(3000000)),
            #[cfg(target_os = "linux")]
            B3500000 => Some(core::BaudOther(3500000)),
            #[cfg(target_os = "linux")]
            B4000000 => Some(core::BaudOther(4000000)),

            _ => None,
        }
    }

    fn char_size(&self) -> Option<core::CharSize> {
        use libc::{CSIZE, CS5, CS6, CS7, CS8};

        match self.termios.c_cflag & CSIZE {
            CS8 => Some(core::Bits8),
            CS7 => Some(core::Bits7),
            CS6 => Some(core::Bits6),
            CS5 => Some(core::Bits5),

            _ => None,
        }
    }

    fn parity(&self) -> Option<core::Parity> {
        use libc::{PARENB, PARODD};

        if self.termios.c_cflag & PARENB != 0 {
            if self.termios.c_cflag & PARODD != 0 {
                Some(core::ParityOdd)
            }
            else {
                Some(core::ParityEven)
            }
        }
        else {
            Some(core::ParityNone)
        }
    }

    fn stop_bits(&self) -> Option<core::StopBits> {
        use libc::CSTOPB;

        if self.termios.c_cflag & CSTOPB != 0 {
            Some(core::Stop2)
        }
        else {
            Some(core::Stop1)
        }
    }

    fn flow_control(&self) -> Option<core::FlowControl> {
        use libc::{IXON, IXOFF};
        use libc::CRTSCTS;

        if self.termios.c_cflag & CRTSCTS != 0 {
            Some(core::FlowHardware)
        }
        else if self.termios.c_iflag & (IXON | IXOFF) != 0 {
            Some(core::FlowSoftware)
        }
        else {
            Some(core::FlowNone)
        }
    }

    fn set_baud_rate(&mut self, baud_rate: core::BaudRate) -> core::Result<()> {
        use libc::EINVAL;
        use libc::{B50, B75, B110, B134, B150, B200, B300, B600, B1200, B1800, B2400, B4800, B9600, B19200, B38400};
        use libc::{B57600, B115200, B230400};

        #[cfg(target_os = "linux")]
        use libc::{B0, B460800, B500000, B576000, B921600, B1000000, B1152000, B1500000, B2000000, B2500000, B3000000, B3500000, B4000000};

        #[cfg(target_os = "macos")]
        use libc::{B7200, B14400, B28800, B76800};

        #[cfg(target_os = "freebsd")]
        use libc::{B7200, B14400, B28800, B76800, B460800, B921600};

        #[cfg(target_os = "openbsd")]
        use libc::{B7200, B14400, B28800, B76800};

        #[cfg(target_os = "linux")]
        use libc::CBAUD;

        let baud = match baud_rate {
            core::BaudOther(50)      => B50,
            core::BaudOther(75)      => B75,
            core::Baud110            => B110,
            core::BaudOther(134)     => B134,
            core::BaudOther(150)     => B150,
            core::BaudOther(200)     => B200,
            core::Baud300            => B300,
            core::Baud600            => B600,
            core::Baud1200           => B1200,
            core::BaudOther(1800)    => B1800,
            core::Baud2400           => B2400,
            core::Baud4800           => B4800,
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            core::BaudOther(7200)    => B7200,
            core::Baud9600           => B9600,
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            core::BaudOther(14400)   => B14400,
            core::Baud19200          => B19200,
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            core::BaudOther(28800)   => B28800,
            core::Baud38400          => B38400,
            core::Baud57600          => B57600,
            #[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd"))]
            core::BaudOther(76800)   => B76800,
            core::Baud115200         => B115200,
            core::BaudOther(230400)  => B230400,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            core::BaudOther(460800)  => B460800,
            #[cfg(target_os = "linux")]
            core::BaudOther(500000)  => B500000,
            #[cfg(target_os = "linux")]
            core::BaudOther(576000)  => B576000,
            #[cfg(any(target_os = "linux", target_os = "freebsd"))]
            core::BaudOther(921600)  => B921600,
            #[cfg(target_os = "linux")]
            core::BaudOther(1000000) => B1000000,
            #[cfg(target_os = "linux")]
            core::BaudOther(1152000) => B1152000,
            #[cfg(target_os = "linux")]
            core::BaudOther(1500000) => B1500000,
            #[cfg(target_os = "linux")]
            core::BaudOther(2000000) => B2000000,
            #[cfg(target_os = "linux")]
            core::BaudOther(2500000) => B2500000,
            #[cfg(target_os = "linux")]
            core::BaudOther(3000000) => B3000000,
            #[cfg(target_os = "linux")]
            core::BaudOther(3500000) => B3500000,
            #[cfg(target_os = "linux")]
            core::BaudOther(4000000) => B4000000,

            core::BaudOther(_) => return Err(super::error::from_raw_os_error(EINVAL)),
        };

        #[cfg(not(target_os = "linux"))]
        unsafe {
            if libc::cfsetspeed(&mut self.termios, baud) < 0 {
                return Err(super::error::last_os_error());
            }
        }

        #[cfg(target_os = "linux")]
        {
            self.termios.c_cflag &= !(CBAUD | CBAUD << IBSHIFT);
            self.termios.c_cflag |= baud | B0 << IBSHIFT;
        }

        Ok(())
    }

    fn set_char_size(&mut self, char_size: core::CharSize) {
        use libc::{CSIZE, CS5, CS6, CS7, CS8};

        let size = match char_size {
            core::Bits5 => CS5,
            core::Bits6 => CS6,
            core::Bits7 => CS7,
            core::Bits8 => CS8,
        };

        self.termios.c_cflag &= !CSIZE;
        self.termios.c_cflag |= size;
    }

    fn set_parity(&mut self, parity: core::Parity) {
        use libc::{PARENB, PARODD, INPCK, IGNPAR};

        match parity {
            core::ParityNone => {
                self.termios.c_cflag &= !(PARENB | PARODD);
                self.termios.c_iflag &= !INPCK;
                self.termios.c_iflag |= IGNPAR;
            }
            core::ParityOdd => {
                self.termios.c_cflag |= PARENB | PARODD;
                self.termios.c_iflag |= INPCK;
                self.termios.c_iflag &= !IGNPAR;
            }
            core::ParityEven => {
                self.termios.c_cflag &= !PARODD;
                self.termios.c_cflag |= PARENB;
                self.termios.c_iflag |= INPCK;
                self.termios.c_iflag &= !IGNPAR;
            }
        };
    }

    fn set_stop_bits(&mut self, stop_bits: core::StopBits) {
        use libc::CSTOPB;

        match stop_bits {
            core::Stop1 => self.termios.c_cflag &= !CSTOPB,
            core::Stop2 => self.termios.c_cflag |= CSTOPB,
        };
    }

    fn set_flow_control(&mut self, flow_control: core::FlowControl) {
        use libc::{IXON, IXOFF};
        use libc::CRTSCTS;

        match flow_control {
            core::FlowNone => {
                self.termios.c_iflag &= !(IXON | IXOFF);
                self.termios.c_cflag &= !CRTSCTS;
            }
            core::FlowSoftware => {
                self.termios.c_iflag |= IXON | IXOFF;
                self.termios.c_cflag &= !CRTSCTS;
            }
            core::FlowHardware => {
                self.termios.c_iflag &= !(IXON | IXOFF);
                self.termios.c_cflag |= CRTSCTS;
            }
        };
    }
}


#[cfg(test)]
mod tests {
    use core;

    use std::mem;

    use super::TTYSettings;
    use core::prelude::*;

    fn default_settings() -> TTYSettings {
        TTYSettings { termios: unsafe { mem::uninitialized() } }
    }

    #[test]
    fn tty_settings_sets_baud_rate() {
        let mut settings = default_settings();

        settings.set_baud_rate(core::Baud600).unwrap();
        assert_eq!(settings.baud_rate(), Some(core::Baud600));
    }

    #[test]
    fn tty_settings_overwrites_baud_rate() {
        let mut settings = default_settings();

        settings.set_baud_rate(core::Baud600).unwrap();
        settings.set_baud_rate(core::Baud1200).unwrap();
        assert_eq!(settings.baud_rate(), Some(core::Baud1200));
    }

    #[test]
    fn tty_settings_sets_char_size() {
        let mut settings = default_settings();

        settings.set_char_size(core::Bits8);
        assert_eq!(settings.char_size(), Some(core::Bits8));
    }

    #[test]
    fn tty_settings_overwrites_char_size() {
        let mut settings = default_settings();

        settings.set_char_size(core::Bits8);
        settings.set_char_size(core::Bits7);
        assert_eq!(settings.char_size(), Some(core::Bits7));
    }

    #[test]
    fn tty_settings_sets_parity_even() {
        let mut settings = default_settings();

        settings.set_parity(core::ParityEven);
        assert_eq!(settings.parity(), Some(core::ParityEven));
    }

    #[test]
    fn tty_settings_sets_parity_odd() {
        let mut settings = default_settings();

        settings.set_parity(core::ParityOdd);
        assert_eq!(settings.parity(), Some(core::ParityOdd));
    }

    #[test]
    fn tty_settings_sets_parity_none() {
        let mut settings = default_settings();

        settings.set_parity(core::ParityEven);
        settings.set_parity(core::ParityNone);
        assert_eq!(settings.parity(), Some(core::ParityNone));
    }

    #[test]
    fn tty_settings_sets_stop_bits_1() {
        let mut settings = default_settings();

        settings.set_stop_bits(core::Stop2);
        settings.set_stop_bits(core::Stop1);
        assert_eq!(settings.stop_bits(), Some(core::Stop1));
    }

    #[test]
    fn tty_settings_sets_stop_bits_2() {
        let mut settings = default_settings();

        settings.set_stop_bits(core::Stop1);
        settings.set_stop_bits(core::Stop2);
        assert_eq!(settings.stop_bits(), Some(core::Stop2));
    }

    #[test]
    fn tty_settings_sets_flow_control_software() {
        let mut settings = default_settings();

        settings.set_flow_control(core::FlowSoftware);
        assert_eq!(settings.flow_control(), Some(core::FlowSoftware));
    }

    #[test]
    fn tty_settings_sets_flow_control_hardware() {
        let mut settings = default_settings();

        settings.set_flow_control(core::FlowHardware);
        assert_eq!(settings.flow_control(), Some(core::FlowHardware));
    }

    #[test]
    fn tty_settings_sets_flow_control_none() {
        let mut settings = default_settings();

        settings.set_flow_control(core::FlowHardware);
        settings.set_flow_control(core::FlowNone);
        assert_eq!(settings.flow_control(), Some(core::FlowNone));
    }
}
