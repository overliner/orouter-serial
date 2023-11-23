use ::std::{thread, time};
use std::str::FromStr;
use std::time::Duration;

use orouter_serial::host::{codec::UsbCodec, Message};

const PORT_NAME: &'static str = "/dev/ttyUSB0";

fn main() {
    let port = serialport::new(PORT_NAME, 9_600)
        .timeout(Duration::from_millis(10))
        .open();

    match port {
        Ok(mut port) => {
            // create a configuration message
            let message = Message::from_str("config@1|7").unwrap();
            // ensure we are sending only frames short enough for selected serial line
            let frames = message.as_frames::<UsbCodec>().unwrap();
            for write_bytes in frames {
                port.write_all(&write_bytes).unwrap();
                thread::sleep(time::Duration::from_millis(250));
            }
        }
        Err(e) => {
            eprintln!("Failed to open \"{}\". Error: {}", PORT_NAME, e);
            ::std::process::exit(1);
        }
    }
}
