# `orouter-serial` (ōRouter serial protocol)

This crate provides typed messages used for serial communication between host and oRouter.
It also contains codecs for different serial HW used in oRouter.

As a wire codec oRouter uses [COBS](https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing)
encoding, specifically [cobs](https://docs.rs/cobs/0.1.4/cobs/index.html) rust crate but this
is completely abstracted from user of this library.

For reading of messages one can use instance of [`crate::host::MessageReader`] -
after it's created it can be fed incoming slices of bytes (`u8`s) and messages are returned
in a [`crate::heapless::Vec`] when successfully decoded.

Example:

```rust
use orouter_serial::message::{
    self,
    SerialMessage,
    StatusCode,
    Status
};

fn main() {
    let mut TEST_DATA: [u8; 4] = [0x03, 0xc5, 0x02, 0x00];
    // try to read message from a buffer
    let messages: Result<Vec<message::SerialMessage>, _>
        = message::try_decode_messages(&mut TEST_DATA[..]).collect();
    println!("received messages = {:?}", messages);
    assert_eq!(
        messages.unwrap().get(0),
        Some(SerialMessage::Status(Status { code: StatusCode::FrameReceived})).as_ref()
    );
}
```

for more complete read example refer to [examples/read.rs](examples/read.rs)

For encoding a message for being send over the serial line a method
[`crate::host::Message::encode`] is used. However, if you know a type of serial line you will
be using to send a message (USB or BLE) you can use [`crate::host::Message::as_frames`] method
with a specific codec from [`crate::host::codec`] module to get message bytes framed in a way
needed by the particular serial line type.


Example:

```rust
use std::str::FromStr;
use orouter_serial::{
    message::SerialMessage,
    codec::{UsbCodec, WireCodec},
};

fn main() {
    let mut data_buffer = [0; 255];
    let mut encoded = [0; 270];
    // let's create a configuration message to set oRouter to EU region with spreading factor 7
    let message = SerialMessage::new_from_str("config@1|7|AACC", &mut data_buffer[..]).unwrap();
    // for simplicity just output the bytes
    let bytes = postcard::to_slice_cobs(&message, &mut encoded).unwrap();
    println!("bytes = {:02x?}", bytes);

    // now let's get frames for sending over USB
    let _frames = UsbCodec::get_frames(bytes).unwrap();
}
```

for more complete write example refer to [examples/write.rs](examples/write.rs)

### Cargo `features`

- `std` - on by default, exposes things which need standard library (e.g. `Debug` derives)
- `defmt-impl` - add [`defmt::Format`] derive implementations

## Dependant internal projects

- cli host tool
- node-overline-protocol
- RTNOrouterProtocols
- oRouter Desktop App
- orouter-cli-miner
