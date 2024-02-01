use std::env;

use orouter_serial::message::SerialMessage;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        return Err("Provide 1 argument".into());
    }

    let str_message = args.pop().expect("No str message argument");

    let mut data = [0; 256];
    let message = SerialMessage::new_from_str(str_message.as_str(), &mut data[..])
        .expect(&format!("could not parse message: {}", str_message));

    let mut bytes = [0; 256];
    let mut cobs_bytes = [0; 270];
    let ser = postcard::to_slice(&message, &mut bytes[..]).expect("could not serialize to bytes");
    let cobs = postcard::to_slice_cobs(&message, &mut cobs_bytes[..])
        .expect("could not serialize message to COBS");

    println!(
        "message {:?},\nbytes: {:02x?},\ncobs bytes: {:02x?}",
        &message, ser, cobs
    );
    Ok(())
}
