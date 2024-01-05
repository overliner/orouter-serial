use ::std::{thread, time};
use std::time::Duration;

use orouter_serial::{
    codec::{UsbCodec, WireCodec},
    message::SerialMessage,
};

const PORT_NAME: &'static str = "/dev/ttyUSB0";

fn main() {
    let port = serialport::new(PORT_NAME, 9_600)
        .timeout(Duration::from_millis(10))
        .open();

    match port {
        Ok(mut port) => {
            // create a configuration message
            let mut data = [0; 256];
            let message = SerialMessage::new_from_str("config@1|7", &mut data[..]).unwrap();
            let mut cobs_bytes = [0; 270];
            // ensure we are sending only frames short enough for selected serial line
            let frames = UsbCodec::get_frames(
                postcard::to_slice_cobs(&message, &mut cobs_bytes[..]).unwrap(),
            )
            .unwrap();
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
