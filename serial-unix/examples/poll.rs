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

extern crate serial_unix;
extern crate libc;

use std::io;
use std::path::Path;

use std::io::prelude::*;
use std::os::unix::prelude::*;

fn main() {
    let mut port = serial_unix::TTYPort::open(Path::new("/dev/ttyUSB0")).unwrap();

    let mut fds = vec![libc::pollfd {
        fd: port.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    }];

    loop {
        let retval = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 100) };

        if retval < 0 {
            panic!("{:?}", io::Error::last_os_error());
        }

        if retval > 0 && fds[0].revents & libc::POLLIN != 0 {
            let mut buffer = Vec::<u8>::new();
            port.read_to_end(&mut buffer).unwrap();

            println!("{:?}", buffer);
        }
    }
}
