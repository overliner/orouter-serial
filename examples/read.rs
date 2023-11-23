use std::io;
use std::time::Duration;

use orouter_serial::host::{
    calculate_cobs_overhead,
    codec::{UsbCodec, MAX_USB_FRAME_LENGTH},
    Message, MessageReader, StatusCode, RAWIQ_DATA_LENGTH,
};

const PORT_NAME: &'static str = "/dev/ttyUSB0";

fn main() {
    let port = serialport::new(PORT_NAME, 9_600)
        .timeout(Duration::from_millis(10))
        .open();

    let mut message_reader =
        MessageReader::<{ calculate_cobs_overhead(RAWIQ_DATA_LENGTH) }, 17>::default(); // ceil(4116 / 256)

    match port {
        Ok(mut port) => {
            let mut serial_buf: Vec<u8> = vec![0; 1000];
            loop {
                match port.read(serial_buf.as_mut_slice()) {
                    Ok(t) => {
                        for chunk in serial_buf[..t].chunks(MAX_USB_FRAME_LENGTH) {
                            match message_reader.process_bytes::<UsbCodec>(&chunk) {
                                Ok(messages) => {
                                    println!("received messages = {:?}", messages);
                                }
                                Err(e) => eprintln!("Error while parsing messages = {:?}", e),
                            }
                        }
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                    Err(e) => eprintln!("{:?}", e),
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to open \"{}\". Error: {}", PORT_NAME, e);
            ::std::process::exit(1);
        }
    }
}
