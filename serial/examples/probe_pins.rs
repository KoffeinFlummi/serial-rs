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

extern crate serial;

use std::env;
use std::thread;
use std::time::Duration;

use serial::prelude::*;

const SETTINGS: serial::PortSettings = serial::PortSettings {
    baud_rate:    serial::Baud9600,
    char_size:    serial::Bits8,
    parity:       serial::ParityNone,
    stop_bits:    serial::Stop1,
    flow_control: serial::FlowNone,
};

fn main() {
    for arg in env::args_os().skip(1) {
        let mut port = serial::open(&arg).unwrap();
        println!("opened device {:?}", arg);
        probe_pins(&mut port).unwrap();
    }
}

fn probe_pins<T: SerialPort>(port: &mut T) -> serial::Result<()> {
    try!(port.configure(&SETTINGS));
    try!(port.set_timeout(Duration::from_millis(100)));

    try!(port.set_rts(false));
    try!(port.set_dtr(false));

    let mut rts = false;
    let mut dtr = false;
    let mut toggle = true;

    loop {
        thread::sleep(Duration::from_secs(1));

        if toggle {
            rts = !rts;
            try!(port.set_rts(rts));
        }
        else {
            dtr = !dtr;
            try!(port.set_dtr(dtr));
        }

        println!("RTS={:5?} DTR={:5?} CTS={:5?} DSR={:5?} RI={:5?} CD={:?}",
                 rts,
                 dtr,
                 try!(port.read_cts()),
                 try!(port.read_dsr()),
                 try!(port.read_ri()),
                 try!(port.read_cd()));

        toggle = !toggle;
    }
}
