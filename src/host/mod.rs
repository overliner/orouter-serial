//! Overline host protocol wrapping module
//!
//! Message send between host (computer, phone) connected using either USB or BLE
//!
//! These are not part of Overline specification
//!
//! When application wants to send messages to overline node, it it supposed to use this library
//! for correct communication - especially
//! [`Message::as_frames`](enum.Message.html#method.as_frames)
//! method is crucial for communication to work as it makes sure the messages is COBS encoded
//! AND split to maximum size frames suitable for the node's serial interface
use core::convert::{TryFrom, TryInto};
#[cfg(feature = "std")]
use core::fmt;
use core::str::FromStr;
use heapless::Vec;

pub mod codec;

pub const BLE_SERIAL_DELIMITER: char = '%';
const COBS_SENTINEL: u8 = 0x00;
pub const DEFAULT_MAX_MESSAGE_QUEUE_LENGTH: usize = 3;
pub const RAWIQ_DATA_LENGTH: usize = 2 * 2048; // 2048 u16s

/// Computed as
///
/// ```ignore - not a test
/// 1+longest_message_length => (now RawIq lenght with max data)
/// +
/// 1+ceil(<previous result>/254) = COBS worst overhead
/// +
/// 1 = COBS sentinel (0x00 in our case)
/// ---
/// <result>
/// ```
///
const fn calculate_max_message_length(max_message_size: usize) -> usize {
    // (x+d-1) / d
    1 + max_message_size + 1 + (max_message_size + 254 - 1) / 254 + 1
}
pub const MAX_MESSAGE_LENGTH: usize = calculate_max_message_length(RAWIQ_DATA_LENGTH);
pub type HostMessageVec = Vec<u8, MAX_MESSAGE_LENGTH>;

#[derive(PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Error {
    BufferFull,
    BufferLengthNotSufficient,
    MalformedMessage,
    MessageQueueFull,
    MalformedHex(base16::DecodeError),
    CannotAppendCommand,
    CannotSplitFrames,
}

impl From<base16::DecodeError> for Error {
    fn from(e: base16::DecodeError) -> Error {
        Error::MalformedHex(e)
    }
}

#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
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
#[cfg(feature = "std")]
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
        /// BE encoded
        version_data: u32,
        region: u8,
        receive_queue_size: u8,
        transmit_queue_size: u8,
    },
    /// Node reporting some error state to host
    Status { code: StatusCode },
    /// Firmware upgrade will follow
    UpgradeFirmwareRequest,
    /// Set current time
    SetTimestamp { timestamp: u64 },
    /// Get rawIq data
    GetRawIq,
    /// Node returns raw IQ data to host
    RawIq {
        data: Vec<u8, { RAWIQ_DATA_LENGTH }>,
    },
}

#[cfg(feature = "std")]
impl fmt::Debug for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Message::SendData { data } => write!(f, "SendData {{ data: {:02x?} }}", data),
            Message::ReceiveData { data } => write!(f, "ReceiveData {{ data: {:02x?} }}", data),
            Message::Configure { region } => write!(f, "Configure {{ region: {:02x?} }}", region),
            Message::ReportRequest => write!(f, "ReportRequest"),
            Message::Report {
                sn,
                version_data,
                region,
                receive_queue_size,
                transmit_queue_size,
            } => write!(f, "Report {{ sn: {:?}, version_data: {:?}, region: {:02x?}, receive_queue_size: {:?}, transmit_queue_size: {:?} }}", sn, version_data, region, receive_queue_size, transmit_queue_size),
            Message::Status { code } => write!(f, "Status({:?})", code),
            Message::UpgradeFirmwareRequest => write!(f, "UpgradeFirmwareRequest"),
            Message::SetTimestamp { timestamp } => write!(f, "SetTimestamp({:?})", timestamp),
            Message::GetRawIq => write!(f, "GetRawIq"),
            Message::RawIq { data } => write!(f, "RawIq {{ data: {:02x?} }}", data)
        }
    }
}

#[cfg_attr(feature = "std", derive(Debug))]
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
            "ts" => Ok(Message::SetTimestamp {
                timestamp: val.parse().unwrap(),
            }),
            "get_rawiq" => Ok(Message::GetRawIq),
            "uf" => Ok(Message::UpgradeFirmwareRequest),
            _ => Err(ParseMessageError::InvalidMessage),
        }
    }
}

impl TryFrom<&[u8]> for Message {
    type Error = Error;

    fn try_from(buf: &[u8]) -> Result<Self, Self::Error> {
        match buf[0] {
            0xc0 => Ok(Message::SendData {
                data: Vec::<u8, 255>::from_slice(&buf[1..]).unwrap(),
            }),
            0xc1 => Ok(Message::ReceiveData {
                data: Vec::<u8, 255>::from_slice(&buf[1..]).unwrap(),
            }),
            0xc2 => Ok(Message::Configure { region: buf[1] }),
            0xc3 => Ok(Message::ReportRequest),
            0xc4 => Ok(Message::Report {
                sn: u32::from_be_bytes(buf[1..5].try_into().unwrap()),
                version_data: u32::from_be_bytes(buf[5..9].try_into().unwrap()),
                region: buf[9],
                receive_queue_size: buf[10],
                transmit_queue_size: buf[11],
            }),
            0xc5 => Ok(Message::Status {
                code: buf[1].try_into().unwrap(),
            }),
            0xc6 => Ok(Message::UpgradeFirmwareRequest),
            0xc7 => Ok(Message::SetTimestamp {
                timestamp: u64::from_be_bytes(buf[1..9].try_into().unwrap()),
            }),
            0xc8 => Ok(Message::GetRawIq),
            _ => Err(Error::MalformedMessage),
        }
    }
}

#[allow(clippy::len_without_is_empty)]
impl Message {
    pub fn try_from_cobs(buf: &mut [u8]) -> Result<Message, Error> {
        if buf.is_empty() {
            return Err(Error::MalformedMessage);
        };

        let decoded_len = match cobs::decode_in_place_with_sentinel(buf, COBS_SENTINEL) {
            Ok(len) => len,
            Err(_) => return Err(Error::MalformedMessage),
        };

        Message::try_from(&buf[0..decoded_len])
    }

    pub fn len(&self) -> usize {
        let variable_part_length = match self {
            Message::SendData { data } => data.len(),
            Message::ReceiveData { data } => data.len(),
            Message::Configure { .. } => 1,
            Message::ReportRequest => 0,
            Message::Report { .. } => 11,
            Message::Status { .. } => 1,
            Message::UpgradeFirmwareRequest => 0,
            Message::SetTimestamp { .. } => 8, // 1x u64 timestamp
            Message::GetRawIq => 0,
            Message::RawIq { data } => data.len(),
        };

        1 + variable_part_length
    }

    pub fn as_bytes(&self) -> Vec<u8, MAX_MESSAGE_LENGTH> {
        let mut res = Vec::new();
        match self {
            Message::SendData { data } => {
                res.push(0xc0).unwrap();
                res.extend_from_slice(&data).unwrap();
            }
            Message::ReceiveData { data } => {
                res.push(0xc1).unwrap();
                res.extend_from_slice(&data).unwrap();
            }
            Message::Configure { region } => {
                res.extend_from_slice(&[0xc2, *region]).unwrap();
            }
            Message::ReportRequest => res.push(0xc3).unwrap(),
            Message::Report {
                sn,
                version_data,
                region,
                receive_queue_size,
                transmit_queue_size,
            } => {
                res.push(0xc4).unwrap();
                res.extend_from_slice(&u32::to_be_bytes(*sn)).unwrap();
                res.extend_from_slice(&u32::to_be_bytes(*version_data))
                    .unwrap();
                res.extend_from_slice(&[*region, *receive_queue_size, *transmit_queue_size])
                    .unwrap();
            }
            Message::Status { code } => {
                res.extend_from_slice(&[0xc5, code.clone() as u8]).unwrap();
            }
            Message::UpgradeFirmwareRequest => res.push(0xc6).unwrap(),
            Message::SetTimestamp { timestamp } => {
                res.push(0xc7).unwrap();
                res.extend_from_slice(&u64::to_be_bytes(*timestamp))
                    .unwrap()
            }
            Message::GetRawIq => res.push(0xc8).unwrap(),
            Message::RawIq { data } => {
                res.push(0xc9).unwrap();
                res.extend_from_slice(&data).unwrap();
            }
        };
        res
    }

    pub fn encode(&self) -> Result<HostMessageVec, Error> {
        let mut result = HostMessageVec::new(); // Maximum message length is 256 + cobs overhead
        let mut encoded_len = cobs::max_encoding_length(self.len() + 1);
        result.resize_default(encoded_len).unwrap();
        let mut enc = cobs::CobsEncoder::new(&mut result);
        enc.push(self.as_bytes().as_slice()).unwrap();

        encoded_len = enc.finalize().unwrap();
        result.push(COBS_SENTINEL).unwrap();
        result.truncate(encoded_len + 1_usize);
        Ok(result)
    }

    /// Splits COBS encoded self to frames for sending.
    /// Frames can be send as is over the wire, it itself is a valid host protocol packet
    pub fn as_frames<C: codec::WireCodec>(&self) -> Result<C::Frames, Error> {
        let mut result = self.encode()?;
        let frames = C::get_frames(&mut result[..]).map_err(|_| Error::CannotSplitFrames)?;
        Ok(frames)
    }
}

pub struct MessageReader<const BUFL: usize, const QL: usize> {
    buf: Vec<u8, BUFL>,
}

impl<const BUFL: usize, const QL: usize> MessageReader<BUFL, QL> {
    pub fn new() -> Self {
        Self {
            buf: Vec::<u8, BUFL>::new(),
        }
    }

    pub fn process_bytes<C: codec::WireCodec>(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<Message, QL>, Error> {
        let (bytes, decoded_len) = C::decode_frame(bytes)?;
        if self.buf.len() + decoded_len > (4116 + 260) {
            return Err(Error::BufferFull);
        }
        self.buf.extend(bytes);

        let mut output = Vec::<Message, QL>::new();
        let mut cobs_index: usize = 0;

        if !&self.buf.contains(&COBS_SENTINEL) {
            return Ok(output);
        }
        loop {
            if self.buf[cobs_index] == COBS_SENTINEL {
                match Message::try_from_cobs(&mut self.buf[0..cobs_index]) {
                    Ok(command) => {
                        self.buf = Vec::from_slice(&self.buf[cobs_index + 1..]).unwrap(); // +1 do not include the COBS_SENTINEL
                        cobs_index = 0;
                        if output.len() < QL {
                            output
                                .push(command)
                                .map_err(|_| Error::CannotAppendCommand)?;
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

    // pub fn process_bytes_hex(&mut self, hex_bytes: &[u8]) -> Result<Vec<Message, QL>, Error> {
    //     let mut decoded = Vec::<u8, 64>::new();
    //     decoded.resize_default(64).unwrap();
    //     match base16::decode_slice(&hex_bytes, &mut decoded) {
    //         Ok(decoded_len) => self.process_bytes(&decoded[0..decoded_len]),
    //         Err(e) => Err(Error::MalformedHex(e)),
    //     }
    // }

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

impl<const BUFL: usize, const QL: usize> Default for MessageReader<BUFL, QL> {
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
            12,
            Message::Report {
                region: 0x01,
                sn: 12345678u32,
                version_data: 0x00_01_00_01u32, // firmware version 0.1.0, hw revision 1
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
        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        assert_eq!(
            cr.process_bytes::<codec::UsbCodec>(&[][..]).unwrap().len(),
            0
        );
    }

    #[test]
    fn test_process_with_no_full_message_is_empty() {
        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        assert_eq!(
            cr.process_bytes::<codec::UsbCodec>(&[0x01, 0x02][..])
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn test_single_message_decoding() {
        let encoded = &[0x03, 0xc2, 0xff, 0x00];
        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = cr.process_bytes::<codec::UsbCodec>(&encoded[..]).unwrap();

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

        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = cr
            .process_bytes::<codec::UsbCodec>(&encoded_buffer[..])
            .unwrap();
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
        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let err = cr.process_bytes::<codec::UsbCodec>(&encoded_buffer[..]);
        assert_eq!(err, Err(Error::MessageQueueFull));
    }

    #[test]
    fn test_single_message_encoding_as_cobs_encoded_usb_frames() {
        let expected = &[0x03, 0xc2, 0xff, 0x00];
        let msg = Message::Configure { region: 255u8 };
        let frames = msg.as_frames::<codec::UsbCodec>().unwrap();

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
        let frames = msg.as_frames::<codec::UsbCodec>().unwrap();
        assert_eq!(frames.len(), 2);

        // lets check the the second (last) frame has COBS_SENTINEL at the end
        let last_frame = &frames.last().unwrap();
        assert_eq!(last_frame.last().unwrap(), &COBS_SENTINEL);
    }

    #[test]
    fn test_ltrim_ok() {
        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let buf = b"%DISCONNECT%";
        cr.process_bytes::<codec::UsbCodec>(buf.as_ref()).unwrap();
        let res = cr.ltrim(buf.len());
        assert_eq!(Ok(()), res);
    }

    #[test]
    fn test_ltrim_err() {
        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let buf = b"%DISCONNECT%";
        cr.process_bytes::<codec::UsbCodec>(buf.as_ref()).unwrap();
        let err = cr.ltrim(buf.len() + 1);
        assert_eq!(err, Err(Error::BufferLengthNotSufficient));
    }

    #[test]
    fn test_single_message_encoding_as_cobs_encoded_frames_for_ble() {
        let expected = &[0x03, 0xc2, 0xff, 0x00];
        let msg = Message::Configure { region: 255u8 };
        let hex_frames = msg.as_frames::<codec::Rn4870Codec>().unwrap();

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
        let hex_frames = msg.as_frames::<codec::Rn4870Codec>().unwrap();

        assert_eq!(hex_frames.len(), 1);
        let hex_frame = hex_frames[0].clone();
        let mut cr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = cr
            .process_bytes::<codec::Rn4870Codec>(&hex_frame[1..hex_frame.len() - 1])
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
        let frames = msg.as_frames::<codec::Rn4870Codec>().unwrap();
        assert_eq!(frames.len(), 5);
    }

    #[test]
    fn test_status_code_encoding() {
        let msg = Message::Status {
            code: StatusCode::ErrBusyLoraTransmitting,
        };
        let encoded = msg.encode().unwrap();
        let mut mr = MessageReader::<MAX_MESSAGE_LENGTH, DEFAULT_MAX_MESSAGE_QUEUE_LENGTH>::new();
        let messages = mr.process_bytes::<codec::UsbCodec>(&encoded[..]).unwrap();
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

    #[test]
    fn test_message_parse_ts() {
        let msg = "ts@1629896485".parse::<Message>().unwrap();
        assert_eq!(
            msg,
            Message::SetTimestamp {
                timestamp: 1629896485u64
            }
        );
    }

    #[test]
    fn test_calculate_max_message_length_255() {
        let max_message_lenght = calculate_max_message_length(255);
        assert_eq!(max_message_lenght, 260);
    }

    #[test]
    fn test_calculate_max_message_length_4096() {
        let max_message_lenght = calculate_max_message_length(4096);
        assert_eq!(max_message_lenght, 4116);
    }
}
