//! This crate provides typed messages used for serial communication between host and oRouter.
//! It also contains codecs for different serial HW used in oRouter.
//!
//! As a wire codec oRouter uses [COBS](https://en.wikipedia.org/wiki/Consistent_Overhead_Byte_Stuffing)
//! encoding, specifically [cobs](https://docs.rs/cobs/0.1.4/cobs/index.html) rust crate but this
//! is completely abstracted from user of this library.
//!
//! For reading of messages one can use instance of [`crate::host::MessageReader`] -
//! after it's created it can be fed incoming slices of bytes (`u8`s) and messages are returned
//! in a [`crate::heapless::Vec`] when successfully decoded.
//!
//! Example:
//!
//! ```rust,no_run
//! use orouter_serial::host::{
//!     calculate_cobs_overhead,
//!     codec::{UsbCodec, MAX_USB_FRAME_LENGTH},
//!     Message, MessageReader, StatusCode, RAWIQ_DATA_LENGTH,
//! };
//!
//! const TEST_DATA: [u8; 4] = [0x03, 0xc5, 0x02, 0x00];
//!
//! fn main() {
//!     let mut message_reader =
//!         MessageReader::<{ calculate_cobs_overhead(RAWIQ_DATA_LENGTH) }, 17>::default();
//!     // feed the message_reader with test data representing a Status message
//!     let messages = message_reader.process_bytes::<UsbCodec>(&TEST_DATA[..]).unwrap();
//!     println!("received messages = {:?}", messages);
//!     assert_eq!(
//!         messages.get(0),
//!         Some(Message::Status { code: StatusCode::FrameReceived}).as_ref()
//!     );
//! }
//! ```
//!
//! for more complete read example refer to [examples/read.rs](examples/read.rs)
//!
//! For encoding a message for being send over the serial line a method
//! [`crate::host::Message::encode`] is used. However, if you know a type of serial line you will
//! be using to send a message (USB or BLE) you can use [`crate::host::Message::as_frames`] method
//! with a specific codec from [`crate::host::codec`] module to get message bytes framed in a way
//! needed by the particular serial line type.
//!
//!
//! Example:
//!
//! ```rust,no_run
//! use std::str::FromStr;
//! use orouter_serial::host::{codec::UsbCodec, Message};
//!
//! fn main() {
//!     // let's create a configuration message to set oRouter to EU region with spreading factor 7
//!     let message = Message::from_str("config@1|7").unwrap();
//!     // for simplicity just output the bytes
//!     println!("bytes = {:02x?}", message.encode().unwrap().as_slice());
//!
//!     // now let's get frames for sending over USB
//!     let _frames = message.as_frames::<UsbCodec>().unwrap();
//! }
//! ```
//!
//! for more complete write example refer to [examples/write.rs](examples/write.rs)
//!
//! ## Cargo `features`
//!
//! - `std` - on by default, exposes things which need standard library (e.g. `Debug` derives)
//! - `defmt-impl` - add [`defmt::Format`] derive implementations

#![cfg_attr(any(not(feature = "std"), not(test)), no_std)]

pub mod host;

// include defmt::Format implementations
// we don't want them derive()d in the modules unless defmt-impl feature is set
#[cfg(feature = "defmt-impl")]
pub mod defmt;

// reexport heapless
pub use heapless;

pub(crate) const MAX_LORA_PAYLOAD_LENGTH: usize = 255;
