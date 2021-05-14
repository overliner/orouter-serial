//! Overline host protocol wrapping module
//!
//! Message send between host (computer, phone) connected using either USB or BLE
//!
//! These are not part of Overline specification
//!
//! When application wants to send messages to overline node, it it supposed to use this library
//! for correct communication - especially [`Message::as_cobs_encoded_serial_frames`](enum.Message.html#method.as_cobs_encoded_serial_frames)
//! method is crucial for communication to work as it makes sure the messages is COBS encoded
//! AND split to maximum size frames suitable for the node's serial interface
use core::convert::{TryFrom, TryInto};
use core::fmt;
use core::str::FromStr;
use heapless::Vec;

pub const BLE_SERIAL_DELIMITER: char = '%';
const COBS_SENTINEL: u8 = 0x00;
pub const DEFAULT_MAX_MESSAGE_QUEUE_LENGTH: usize = 3;

/// Computed as
///
/// ```ignore - not a test
/// 1+255 => longest message raw bytes length (SendData.len() when data vec is full)
/// +
/// 1+ceil(256/254) = 4 = COBS worst overhead
/// +
/// 1 = COBS sentinel
/// ---
/// 260
/// ```
///
pub const MAX_MESSAGE_LENGTH: usize = 260;
pub const MAX_MESSAGE_LENGTH_HEX_ENCODED: usize = 2 * MAX_MESSAGE_LENGTH; // hex encoding - each byte = 2 chars
pub type HostMessageVec = Vec<u8, MAX_MESSAGE_LENGTH>;
/// cannot run calculation in const declaration
/// calculation is min(1, MAX_MESSAGE_LENGTH_HEX_ENCODED % MAX_SERIAL_FRAME_LENGTH);
/// which is min(1, 520 % 128) = min(1, 8) = 1
const MAX_HEX_ENCODED_FRAMES_COUNT_REMAINDER: usize = 1;

pub const MAX_HEX_ENCODED_FRAMES_COUNT: usize = MAX_MESSAGE_LENGTH_HEX_ENCODED
    / MAX_SERIAL_FRAME_LENGTH
    + MAX_HEX_ENCODED_FRAMES_COUNT_REMAINDER;

const MAX_SERIAL_FRAME_LENGTH: usize = 128; // BLE can only process this
type SerialFrameVec = Vec<u8, MAX_SERIAL_FRAME_LENGTH>;

#[derive(Debug, PartialEq)]
pub enum Error {
    BufferFull,
    BufferLengthNotSufficient,
    MalformedMessage,
    MessageQueueFull,
    MalformedHex(base16::DecodeError),
}

impl From<base16::DecodeError> for Error {
    fn from(e: base16::DecodeError) -> Error {
        Error::MalformedHex(e)
    }
}

#[derive(Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum StatusCode {
    FrameReceived = 1,
    CommandReceived = 2,
    ErrUnknownCommmandReceived = 3,
    ErrBusyLoraTransmitting = 4,
    ErrMessageQueueFull = 5,
}

impl TryFrom<u8> for StatusCode {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(StatusCode::FrameReceived),
            2 => Ok(StatusCode::CommandReceived),
            3 => Ok(StatusCode::ErrUnknownCommmandReceived),
            4 => Ok(StatusCode::ErrBusyLoraTransmitting),
            5 => Ok(StatusCode::ErrMessageQueueFull),
            _ => Err("Unknown StatusCode"),
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            StatusCode::FrameReceived => write!(f, "Single serial frame was successfuly received"),
            StatusCode::CommandReceived => write!(f, "Command was successfully received"),
            StatusCode::ErrUnknownCommmandReceived => write!(f, "Error: unknown command type"),
            StatusCode::ErrBusyLoraTransmitting => write!(
                f,
                "Error: cannot execute sent command - radio is currently busy transmitting"
            ),
            StatusCode::ErrMessageQueueFull => write!(
                f,
                "Transmit queue is full, try sending SendData later again"
            ),
        }
    }
}

/// Possible commands send over host protocol
///
/// This enum contains both messages send exlusively to node or exclusively to host
#[derive(PartialEq)]
pub enum Message {
    /// Host sending data to node instructing it to broadcast it to the wireless network
    SendData {
        data: Vec<u8, { crate::overline::MAX_LORA_PAYLOAD_LENGTH }>,
    },
    /// Node sending data to host
    ReceiveData {
        data: Vec<u8, { crate::overline::MAX_LORA_PAYLOAD_LENGTH }>,
    },
    /// Host is recongifuring the node
    Configure { region: u8 },
    /// Host requesting the node status
    ReportRequest,
    /// Node reporting information to host
    Report {
        /// BE encoded
        sn: u32,
        region: u8,
        receive_queue_size: u8,
        transmit_queue_size: u8,
    },
    /// Node reporting some error state to host
    Status { code: StatusCode },
    /// Request noise values from node
    GetNoise,
    /// Node reports noise values to host
    Noise { rssi_value: u8, rssi_wideband: u8 },
    /// Firmware upgrade will follow
    UpgradeFirmwareRequest,
}

impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Message::SendData { data } => write!(f, "SendData {{ data: {:02x?} }}", data),
            Message::ReceiveData { data } => write!(f, "ReceiveData {{ data: {:02x?} }}", data),
            Message::Configure { region } => write!(f, "Configure {{ region: {:02x?} }}", region),
            Message::ReportRequest => write!(f, "ReportRequest"),
            Message::Report {
                sn,
                region,
                receive_queue_size,
                transmit_queue_size,
            } => write!(f, "Report {{ sn: {:?}, region: {:02x?}, receive_queue_size: {:?}, transmit_queue_size: {:?} }}", sn, region, receive_queue_size, transmit_queue_size),
            Message::Status { code } => write!(f, "Status({:?})", code),
            Message::GetNoise => write!(f, "GetNoise"),
            Message::Noise { rssi_value, rssi_wideband } => write!(f, "Noise {{ rssi_value: {:?}, rssi_wideband: {:?} }}", rssi_value, rssi_wideband),
            Message::UpgradeFirmwareRequest => write!(f, "UpgradeFirmwareRequest"),
        }
    }
}

#[derive(Debug)]
pub enum ParseMessageError {
    MissingSeparator,
    InvalidMessage,
    InvalidHex(base16::DecodeError),
    InvalidPayloadLength,
    PayloadTooLong,
}

impl From<base16::DecodeError> for ParseMessageError {
    fn from(e: base16::DecodeError) -> ParseMessageError {
        ParseMessageError::InvalidHex(e)
    }
}

impl FromStr for Message {
    type Err = ParseMessageError;

    fn from_str(s: &str) -> Result<Self, ParseMessageError> {
        if !s.contains('@') {
            return Err(ParseMessageError::MissingSeparator);
        }

        let mut iter = s.split(|c| c == '@');
        let cmd_type = iter.next().unwrap();
        let val = iter.next().unwrap();
        match cmd_type {
            "send" => {
                let mut data = Vec::<u8, { crate::overline::MAX_LORA_PAYLOAD_LENGTH }>::new();
                let clean_val = match val.starts_with("0x") || val.starts_with("0X") {
                    true => &val[2..],
                    false => &val,
                };
                if clean_val.len() / 2 > crate::overline::MAX_LORA_PAYLOAD_LENGTH {
                    return Err(ParseMessageError::PayloadTooLong);
                }
                data.resize_default(clean_val.len() / 2)
                    .map_err(|_| ParseMessageError::InvalidPayloadLength)?;
                if base16::decode_slice(clean_val, &mut data).is_err() {
                    return Err(ParseMessageError::InvalidPayloadLength);
                }

                Ok(Message::SendData { data })
            }
            "status" => Ok(Message::ReportRequest),
            "config" => {
                let region = u8::from_str(val).unwrap();
                Ok(Message::Configure { region })
            }
            "get_noise" => Ok(Message::GetNoise),
            "uf" => Ok(Message::UpgradeFirmwareRequest),
            _ => Err(ParseMessageError::InvalidMessage),
        }
    }
}

#[allow(clippy::len_without_is_empty)]
impl Message {
    pub fn try_from(buf: &mut [u8]) -> Result<Message, Error> {
        if buf.is_empty() {
            return Err(Error::MalformedMessage);
        };

        let decoded_len = match cobs::decode_in_place_with_sentinel(buf, COBS_SENTINEL) {
            Ok(len) => len,
            Err(_) => return Err(Error::MalformedMessage),
        };

        match buf[0] {
            0xc0 => Ok(Message::SendData {
                data: Vec::<u8, 255>::from_slice(&buf[1..decoded_len]).unwrap(),
            }),
            0xc1 => Ok(Message::ReceiveData {
                data: Vec::<u8, 255>::from_slice(&buf[1..decoded_len]).unwrap(),
            }),
            0xc2 => Ok(Message::Configure { region: buf[1] }),
            0xc3 => Ok(Message::ReportRequest),
            0xc4 => Ok(Message::Report {
                sn: u32::from_be_bytes(buf[1..5].try_into().unwrap()),
                region: buf[5],
                receive_queue_size: buf[6],
                transmit_queue_size: buf[7],
            }),
            0xc5 => Ok(Message::Status {
                code: buf[1].try_into().unwrap(),
            }),
            0xc6 => Ok(Message::GetNoise),
            0xc7 => Ok(Message::Noise {
                rssi_value: buf[1],
                rssi_wideband: buf[2],
            }),
            0xc8 => Ok(Message::UpgradeFirmwareRequest),
            _ => Err(Error::MalformedMessage),
        }
    }

    pub fn len(&self) -> usize {
        let variable_part_length = match self {
            Message::SendData { data } => data.len(),
            Message::ReceiveData { data } => data.len(),
            Message::Configure { .. } => 1,
            Message::ReportRequest => 0,
            Message::Report { .. } => 7,
            Message::Status { .. } => 1,
            Message::GetNoise => 0,
            Message::Noise { .. } => 2,
            Message::UpgradeFirmwareRequest => 0,
        };

        1 + variable_part_length
    }

    pub fn encode(&self) -> Result<HostMessageVec, Error> {
        let mut result = HostMessageVec::new(); // Maximum message length is 256 + cobs overhead
        let mut encoded_len = cobs::max_encoding_length(self.len() + 1);
        result.resize_default(encoded_len).unwrap();
        let mut enc = cobs::CobsEncoder::new(&mut result);
        match self {
            Message::SendData { data } => {
                enc.push(&[0xc0]).unwrap();
                enc.push(&data).unwrap();
            }
            Message::ReceiveData { data } => {
                enc.push(&[0xc1]).unwrap();
                enc.push(&data).unwrap();
            }
            Message::Configure { region } => {
                enc.push(&[0xc2, *region]).unwrap();
            }
            Message::ReportRequest => enc.push(&[0xc3]).unwrap(),
            Message::Report {
                sn,
                region,
                receive_queue_size,
                transmit_queue_size,
            } => {
                enc.push(&[0xc4]).unwrap();
                enc.push(&u32::to_be_bytes(*sn)).unwrap();
                enc.push(&[*region]).unwrap();
                enc.push(&[*receive_queue_size]).unwrap();
                enc.push(&[*transmit_queue_size]).unwrap();
            }
            Message::Status { code } => {
                enc.push(&[0xc5, code.clone() as u8]).unwrap();
            }
            Message::GetNoise => enc.push(&[0xc6]).unwrap(),
            Message::Noise {
                rssi_value,
                rssi_wideband,
            } => {
                enc.push(&[0xc7]).unwrap();
                enc.push(&[*rssi_value]).unwrap();
                enc.push(&[*rssi_wideband]).unwrap();
            }
            Message::UpgradeFirmwareRequest => enc.push(&[0xc8]).unwrap(),
        };

        encoded_len = enc.finalize().unwrap();
        result.push(COBS_SENTINEL).unwrap();
        result.truncate(encoded_len + 1_usize);
        Ok(result)
    }

    /// Serializes messages using COBS encoding and DOES terminate it with COBS_SENTINEL
    /// Returned Vecs can be send as is over the wire, it itself is a valid host protocol packet
    pub fn as_cobs_encoded_serial_frames(
        &self,
    ) -> Result<Vec<SerialFrameVec, MAX_HEX_ENCODED_FRAMES_COUNT>, Error> {
        let mut result = self.encode().unwrap();
        let mut frames = Vec::<SerialFrameVec, MAX_HEX_ENCODED_FRAMES_COUNT>::new();
        for chunk in result.chunks_mut(MAX_SERIAL_FRAME_LENGTH) {
            frames
                .push(SerialFrameVec::from_slice(&chunk).unwrap())
                .unwrap()
        }
        Ok(frames)
    }

    /// Returns frames inteded to be send over our BLE connected which only is capable of
    /// correctly decode messages which contain only ASCII - don't ask, just read section 2.4.3 of
    /// RN4870-71 User Guide (DS50002466C)
    pub fn as_cobs_encoded_frames_for_ble(
        &self,
        ble_serial_delimiter: char,
    ) -> Result<Vec<SerialFrameVec, MAX_HEX_ENCODED_FRAMES_COUNT>, Error> {
        let result = self.encode().unwrap();
        let mut hex_result = Vec::<u8, MAX_MESSAGE_LENGTH_HEX_ENCODED>::new();
        hex_result.resize_default(result.len() * 2).unwrap();
        base16::encode_config_slice(&result, base16::EncodeLower, &mut hex_result);

        // wrap each chunk in a delimiter char
        let mut frames = Vec::<SerialFrameVec, MAX_HEX_ENCODED_FRAMES_COUNT>::new();
        for chunk in hex_result.chunks_mut(MAX_SERIAL_FRAME_LENGTH - 2) {
            let mut frame = SerialFrameVec::new();
            frame.push(ble_serial_delimiter as u8).unwrap();
            frame.extend_from_slice(&chunk).unwrap();
            frame.push(ble_serial_delimiter as u8).unwrap();
            frames.push(frame).unwrap()
        }
        Ok(frames)
    }
}

pub struct MessageReader<const QL: usize> {
    buf: Vec<u8, 792>,
}

impl<const QL: usize> MessageReader<QL> {
    pub fn new() -> Self {
        Self {
            buf: Vec::<u8, 792>::new(),
        }
    }

    pub fn process_bytes(&mut self, bytes: &[u8]) -> Result<Vec<Message, QL>, Error> {
        self.buf
            .extend_from_slice(bytes)
            .map_err(|_| Error::BufferFull)?;

        let mut output = Vec::<Message, QL>::new();
        let mut cobs_index: usize = 0;

        if !&self.buf.contains(&COBS_SENTINEL) {
            return Ok(output);
        }
        loop {
            if self.buf[cobs_index] == COBS_SENTINEL {
                match Message::try_from(&mut self.buf[0..cobs_index]) {
                    Ok(command) => {
                        self.buf = Vec::from_slice(&self.buf[cobs_index + 1..]).unwrap(); // +1 do not include the COBS_SENTINEL
                        cobs_index = 0;
                        if output.len() < QL {
                            output.push(command).unwrap();
                        } else {
                            return Err(Error::MessageQueueFull);
                        }
                        if self.buf.len() == 0 {
                            break;
                        } else {
                            continue;
                        }
                    }
                    Err(_) => return Err(Error::MalformedMessage),
                }
            }

            if cobs_index + 1 == self.buf.len() {
                break;
            }

            cobs_index += 1;
        }
        Ok(output)
    }

    pub fn process_bytes_hex(&mut self, hex_bytes: &[u8]) -> Result<Vec<Message, QL>, Error> {
        let mut decoded = Vec::<u8, 64>::new();
        decoded.resize_default(64).unwrap();
        match base16::decode_slice(&hex_bytes, &mut decoded) {
            Ok(decoded_len) => self.process_bytes(&decoded[0..decoded_len]),
            Err(e) => Err(Error::MalformedHex(e)),
        }
    }

    pub fn ltrim(&mut self, length: usize) -> Result<(), Error> {
        if self.buf.len() < length {
            return Err(Error::BufferLengthNotSufficient);
        }

        self.buf = match Vec::from_slice(&self.buf[length..]) {
            Ok(b) => b,
            Err(_) => return Err(Error::BufferLengthNotSufficient),
        };
        Ok(())
    }

    pub fn reset(&mut self) {
        self.buf.clear();
    }
}

impl<const QL: usize> Default for MessageReader<QL> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{thread_rng, Rng};

    #[test]
    fn test_msg_len() {
        assert_eq!(
            8,
            Message::Report {
                region: 0x01,
                sn: 12345678u32,
                receive_queue_size: 1,
                transmit_queue_size: 3
            }
            .len()
        );
        assert_eq!(1, Message::ReportRequest.len());
        assert_eq!(2, Message::Configure { region: 0x1 }.len());
        assert_eq!(
            3,
            Message::SendData {
                data: Vec::<u8, 255>::from_slice(&[0xff, 0xee]).unwrap()
            }
            .len()
        );
        assert_eq!(
            5,
            Message::ReceiveData {
                data: Vec::<u8, 255>::from_slice(&[0xde, 0xad, 0xbe, 0xef]).unwrap()
            }
            .len()
        );
    }

    #[test]
    fn test_process_with_no_bytes_is_empty() {
        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        assert_eq!(cr.process_bytes(&[][..]).unwrap().len(), 0);
    }

    #[test]
    fn test_process_with_no_full_message_is_empty() {
        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        assert_eq!(cr.process_bytes(&[0x01, 0x02][..]).unwrap().len(), 0);
    }

    #[test]
    fn test_broken_case_1() {
        let encoded = &[
            0xaa, 0xd6, 0xbb, 0x28, 0x44, 0xf8, 0x47, 0xfd, 0xf7, 0xd8, 0x25, 0xfe, 0x74, 0x07,
            0xd6, 0x39, 0x1a, 0xce, 0xcd, 0xa2, 0xfb, 0xbf, 0xa1, 0xe0, 0x26, 0x60, 0x49, 0x7b,
            0x84, 0x97, 0x5b, 0x75, 0x5d, 0xd8, 0xe7, 0x3e, 0xd6, 0x25, 0x90, 0x5a, 0x21, 0x03,
            0x77, 0x68, 0xcf, 0xf2, 0xff, 0x5c, 0xe3, 0x5c, 0x77, 0x59, 0x6e, 0x59, 0x0c, 0xcc,
            0x23, 0x44, 0x1e, 0xe4, 0x78, 0x4d, 0xe7, 0x97, 0x13, 0x4d, 0xe9, 0x2e, 0xc0, 0x8b,
            0xb0, 0x46, 0xd2, 0x3a, 0x27, 0x3a, 0xd5, 0x2f, 0xdb, 0x96, 0x29, 0x92, 0x2f, 0x5e,
            0x79, 0x9f, 0x6f, 0x66, 0x6b, 0x6d, 0xd7, 0xa9, 0x7f, 0x0f, 0xae, 0x64, 0x75, 0x80,
            0x2b, 0xca, 0xba, 0xd7, 0xf6, 0x8c, 0x1c, 0xcf, 0xe9, 0x67, 0xb6, 0xdb, 0x1a, 0x27,
            0x10, 0x3a, 0xf3, 0xa4, 0x1d, 0x00, 0xb2, 0x6d, 0x1e, 0x48, 0x59, 0xaf, 0x28, 0x1a,
            0x43, 0x3d, 0xe9, 0x9e, 0xe6, 0xc5, 0x06, 0xdd, 0x63, 0x9a, 0x1c, 0x72, 0xb9, 0x3f,
            0x76, 0x96, 0x63, 0xf4, 0x8a, 0x5b, 0x7b, 0x3a, 0xb2, 0xd8, 0x9f, 0x90, 0x98, 0xfc,
            0x49, 0x71, 0x1d, 0x79, 0xae, 0x88, 0x74, 0x1a, 0xe7, 0xdf, 0x43, 0x04, 0x66, 0xd3,
            0xe5, 0x24, 0x92, 0xec, 0xde, 0xe4, 0x15, 0x4b, 0x4d, 0xbe, 0x09, 0x02, 0x13, 0x41,
            0x2a, 0xcf, 0x38, 0xe8, 0x01, 0x91, 0xb5, 0x1b, 0xa8, 0xc5, 0xcd, 0xbb, 0xa8, 0x3a,
            0xaa, 0xd0, 0x80, 0xf7, 0x80, 0xee, 0x64, 0xde, 0xa8, 0xe7, 0xa4, 0xd0, 0x47, 0x05,
            0xdc, 0x50, 0xf6, 0x33, 0x40, 0xe8, 0x90, 0xaa, 0x7a, 0xe5, 0x71, 0x32, 0x1a, 0x2a,
            0xfd, 0xc7, 0x4b, 0x3d, 0x85, 0xb2, 0x0d, 0x58, 0x09, 0xdb, 0xaf, 0x70, 0x31, 0x22,
            0xf1, 0x1d, 0x92, 0x81, 0x19, 0x44, 0x92, 0xe5, 0x8d, 0xb5, 0xad, 0x64, 0x24, 0x7b,
            0xf4, 0x3b, 0xf8,
        ];
        let msg = Message::SendData {
            data: Vec::<u8, 255>::from_slice(&encoded[..]).unwrap(),
        };

        let frames = msg.as_cobs_encoded_serial_frames().unwrap();

        assert_eq!(frames.len(), 3);

        let result = &frames[0];
        let last_frame = &frames.last().unwrap();
        assert_eq!(result.len(), MAX_SERIAL_FRAME_LENGTH);
        for b in &result[0..result.len() - 2] {
            assert_ne!(0x00, *b);
        }
        assert_eq!(Some(&0x00), last_frame.last());
    }

    #[test]
    fn test_single_message_decoding() {
        let encoded = &[0x03, 0xc2, 0xff, 0x00];
        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = cr.process_bytes(&encoded[..]).unwrap();

        let expected_msg_0 = Message::Configure { region: 255u8 };
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], expected_msg_0);
    }

    #[test]
    fn test_multiple() {
        let mut encoded_buffer = [128; 32];
        let mut start = 0;
        for msg in vec![vec![0xc0, 0xff, 0xee], vec![0xc1, 0xde, 0xad, 0xbe, 0xef]] {
            println!("start index is = {}", start);
            let written = cobs::encode(&msg, &mut encoded_buffer[start..]);
            println!("encoded_buffer -> {:02x?}", encoded_buffer);
            encoded_buffer[start + written] = 0x00;
            println!(
                "start = {}, written = {}\nencoded_buffer -> {:02x?}",
                start,
                written + 1,
                encoded_buffer
            );
            start = start + written + 1;
        }

        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = cr.process_bytes(&encoded_buffer[..]).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages[0],
            Message::SendData {
                data: Vec::<u8, 255>::from_slice(&[0xff, 0xee]).unwrap()
            }
        );
        assert_eq!(
            messages[1],
            Message::ReceiveData {
                data: Vec::<u8, 255>::from_slice(&[0xde, 0xad, 0xbe, 0xef]).unwrap()
            }
        );
    }

    // TODO test the rest of the message types
    #[test]
    fn test_more_than_queue_capacity() {
        let mut encoded_buffer = [128; 32];
        let mut start = 0;
        for msg in vec![
            vec![0xc0, 0xff, 0xee],
            vec![0xc0, 0xff, 0xee],
            vec![0xc0, 0xff, 0xee],
            vec![0xc0, 0xff, 0xee],
            vec![0xc0, 0xff, 0xee],
        ] {
            println!("start index is = {}", start);
            let written = cobs::encode(&msg, &mut encoded_buffer[start..]);
            println!("encoded_buffer -> {:02x?}", encoded_buffer);
            encoded_buffer[start + written] = 0x00;
            println!(
                "start = {}, written = {}\nencoded_buffer -> {:02x?}",
                start,
                written + 1,
                encoded_buffer
            );
            start = start + written + 1;
        }
        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let err = cr.process_bytes(&encoded_buffer[..]);
        assert_eq!(err, Err(Error::MessageQueueFull));
    }

    #[test]
    fn test_single_message_encoding_as_cobs_encoded_serial_frames() {
        let expected = &[0x03, 0xc2, 0xff, 0x00];
        let msg = Message::Configure { region: 255u8 };
        let frames = msg.as_cobs_encoded_serial_frames().unwrap();

        assert_eq!(frames.len(), 1);
        let result = &frames[0];
        println!("encoded = {:02x?}", &result);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_max_len_data_message_encoding() {
        let mut arr = [0u8; crate::overline::MAX_LORA_PAYLOAD_LENGTH];
        thread_rng().try_fill(&mut arr[..]).unwrap();

        let msg = Message::SendData {
            data: Vec::<u8, { crate::overline::MAX_LORA_PAYLOAD_LENGTH }>::from_slice(&arr)
                .unwrap(),
        };

        // msg get encoded to more than MaxSerialFrameLength so we should get 2 frames
        let frames = msg.as_cobs_encoded_serial_frames().unwrap();
        assert_eq!(frames.len(), 3);

        // lets check the the second (last) frame has COBS_SENTINEL at the end
        let last_frame = &frames.last().unwrap();
        assert_eq!(last_frame.last().unwrap(), &COBS_SENTINEL);
    }

    #[test]
    fn test_ltrim_ok() {
        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let buf = b"%DISCONNECT%";
        cr.process_bytes(buf.as_ref()).unwrap();
        let res = cr.ltrim(buf.len());
        assert_eq!(Ok(()), res);
    }

    #[test]
    fn test_ltrim_err() {
        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let buf = b"%DISCONNECT%";
        cr.process_bytes(buf.as_ref()).unwrap();
        let err = cr.ltrim(buf.len() + 1);
        assert_eq!(err, Err(Error::BufferLengthNotSufficient));
    }

    #[test]
    fn test_single_message_encoding_as_cobs_encoded_frames_for_ble() {
        let expected = &[0x03, 0xc2, 0xff, 0x00];
        let msg = Message::Configure { region: 255u8 };
        let hex_frames = msg.as_cobs_encoded_frames_for_ble('%').unwrap();

        assert_eq!(hex_frames.len(), 1);
        let hex_frame = &hex_frames[0];
        let mut decoded = Vec::<u8, 4>::new();
        decoded.resize_default(expected.len()).unwrap();
        base16::decode_slice(&hex_frame.clone()[1..hex_frame.len() - 1], &mut decoded).unwrap();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn test_message_reader_process_bytes_hex() {
        let msg = Message::Configure { region: 255u8 };
        let hex_frames = msg.as_cobs_encoded_frames_for_ble('%').unwrap();

        assert_eq!(hex_frames.len(), 1);
        let hex_frame = hex_frames[0].clone();
        let mut cr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = cr
            .process_bytes_hex(&hex_frame[1..hex_frame.len() - 1])
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], msg);
    }

    #[test]
    fn test_max_message_length_as_cobs_encoded_frames_for_ble() {
        let mut arr = [0u8; crate::overline::MAX_LORA_PAYLOAD_LENGTH];
        thread_rng().try_fill(&mut arr[..]).unwrap();

        let msg = Message::SendData {
            data: Vec::<u8, { crate::overline::MAX_LORA_PAYLOAD_LENGTH }>::from_slice(&arr)
                .unwrap(),
        };

        // msg get encoded to more than MaxSerialFrameLength so we should get 5 frames
        let frames = msg
            .as_cobs_encoded_frames_for_ble(BLE_SERIAL_DELIMITER)
            .unwrap();
        assert_eq!(frames.len(), MAX_HEX_ENCODED_FRAMES_COUNT);
    }

    #[test]
    fn test_status_code_encoding() {
        let msg = Message::Status {
            code: StatusCode::ErrBusyLoraTransmitting,
        };
        let encoded = msg.encode().unwrap();
        let mut mr = MessageReader::<DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = mr.process_bytes(&encoded[..]).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0], msg);
    }

    #[test]
    fn test_message_parse_status() {
        let msg = "status@".parse::<Message>().unwrap();
        assert_eq!(msg, Message::ReportRequest);
    }

    #[test]
    fn test_message_parse_send_data() {
        let msg = "send@0xAABB".parse::<Message>().unwrap();
        assert_eq!(
            msg,
            Message::SendData {
                data: Vec::<u8, { crate::overline::MAX_LORA_PAYLOAD_LENGTH }>::from_slice(&[
                    0xaa, 0xbb
                ])
                .unwrap()
            }
        );

        let msg = "send@ccdd".parse::<Message>().unwrap();
        assert_eq!(
            msg,
            Message::SendData {
                data: Vec::<u8, { crate::overline::MAX_LORA_PAYLOAD_LENGTH }>::from_slice(&[
                    0xcc, 0xdd
                ])
                .unwrap()
            }
        );
    }

    #[test]
    fn test_message_parse_config() {
        let msg = "config@1".parse::<Message>().unwrap();
        assert_eq!(msg, Message::Configure { region: 1 });
    }
}
