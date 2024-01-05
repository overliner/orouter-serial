//! Serial messages defitions
//!
//! Messages are of two types - depending on direction they are sent. One type goes from host to
//! orouter and the other vice versa.
#[cfg(feature = "std")]
use core::fmt;
#[cfg(feature = "std")]
use core::str::FromStr;

use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "std", derive(Debug))]
pub enum Error {
    InvalidMessageType(u8),
    MissingCobsSentinel,
    CantCreatePayloadRef,
    LoraDataTooLong(usize),
    InvalidStatusCode(u8),
    BufferToShortToEncodeMessage,
    CantWriteMessageToBuffer,
    RawIqDataIncorrectLength,
}

#[cfg_attr(feature = "std", derive(Debug, PartialEq))]
pub enum FromStrError<'a> {
    MissingSeparator,
    InvalidMessage,
    InvalidHex(base16::DecodeError),
    InvalidHexConfigNetwork,
    InvalidPayloadLength,
    PayloadTooLong,
    MissingConfigNetwork,
    InvalidTargetBuffer,
    InvalidTimestamp(&'a str),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
#[serde(bound(deserialize = "'de: 'a"))]
/// Represents a command sent over serial interface to a from oRouter
pub enum SerialMessage<'a> {
    /// Host sending data to node instructing it to broadcast it to the wireless network
    SendData(SendData<'a>),
    /// Host is recongifuring the node
    Configure(Configure),
    /// Host requesting the node status
    ReportRequest(ReportRequest),
    /// Firmware upgrade will follow
    UpgradeFirmwareRequest,
    /// Set current time
    SetTimestamp(SetTimestamp),
    GetRawIq,
    /// Node sending data to host
    ReceiveData(ReceiveData<'a>),
    /// Node reporting information to host
    Report(Report),
    /// Node reporting some error state to host
    Status(Status),
    /// Node returns raw IQ data to host
    RawIq(RawIq<'a>),
}

#[cfg(feature = "std")]
impl<'a> SerialMessage<'a> {
    /// Implements [`core::str::FromStr`] functionality, but `FromStr::from_str`
    /// signature doesn't have lifetimes and a buffer for parsed data (in case of
    /// [`crate::message::SerialMessage::SendData`] variant) has to be passed as argument.
    pub fn new_from_str(s: &'a str, data: &'a mut [u8]) -> Result<Self, FromStrError<'a>> {
        if !s.contains('@') {
            return Err(FromStrError::MissingSeparator);
        }

        let mut iter = s.split(|c| c == '@');
        let cmd_type = iter.next().unwrap();
        let val = iter.next().unwrap();
        match cmd_type {
            "send" => {
                let clean_val = match val.starts_with("0x") || val.starts_with("0X") {
                    true => &val[2..],
                    false => &val,
                };
                if clean_val.len() / 2 > crate::MAX_LORA_PAYLOAD_LENGTH {
                    return Err(FromStrError::PayloadTooLong);
                }
                let len = match base16::decode_slice(clean_val, data) {
                    Ok(len) => len,
                    Err(_) => return Err(FromStrError::InvalidPayloadLength),
                };

                Ok(SerialMessage::SendData(SendData {
                    data: &data[0..len],
                }))
            }
            "status" => Ok(SerialMessage::ReportRequest(ReportRequest)),
            "config" => {
                if !val.contains('|') {
                    return Err(FromStrError::MissingSeparator);
                }
                let mut iter = val.split(|c| c == '|');
                let region = iter.next().unwrap();
                let region = u8::from_str(region).unwrap();
                let spreading_factor = iter.next().unwrap();
                let spreading_factor = u8::from_str(spreading_factor).unwrap();
                if spreading_factor < 7 || spreading_factor > 12 {
                    return Err(FromStrError::InvalidMessage);
                }
                let network = iter.next().ok_or(FromStrError::MissingConfigNetwork)?;
                let network = u16::from_str_radix(network, 16)
                    .map_err(|_| FromStrError::InvalidHexConfigNetwork)
                    .map(u16::to_be_bytes)?;
                Ok(SerialMessage::Configure(Configure {
                    region,
                    spreading_factor,
                    network,
                }))
            }
            "ts" => Ok(SerialMessage::SetTimestamp(SetTimestamp {
                timestamp: val.parse().map_err(|_| FromStrError::InvalidTimestamp(s))?,
            })),
            "get_rawiq" => Ok(SerialMessage::GetRawIq),
            "uf" => Ok(SerialMessage::UpgradeFirmwareRequest),
            _ => Err(FromStrError::InvalidMessage),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SendData<'a> {
    pub data: &'a [u8],
}

#[cfg(feature = "std")]
impl<'a> core::fmt::Debug for SendData<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SendData {{ data: {:02x?} }}", self.data)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Configure {
    pub region: u8,
    pub spreading_factor: u8,
    pub network: [u8; 2],
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ReportRequest;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct UpgradeFirmwareRequest;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct SetTimestamp {
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct GetRawIq;

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ReceiveData<'a> {
    pub data: &'a [u8],
}

#[cfg(feature = "std")]
impl<'a> core::fmt::Debug for ReceiveData<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ReceiveData {{ data: {:02x?} }}", self.data)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Report {
    pub sn: u32,
    pub version_data: [u8; 9],
    pub region: u8,
    pub spreading_factor: u8,
    pub receive_queue_size: u8,
    pub transmit_queue_size: u8,
    pub network: [u8; 2],
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Status {
    pub code: StatusCode,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(u8)]
pub enum StatusCode {
    FrameReceived = 1,
    CommandReceived = 2,
    ErrUnknownCommmandReceived = 3,
    ErrBusyLoraTransmitting = 4,
    ErrMessageQueueFull = 5,
    RadioNotConfigured = 6,
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
            6 => Ok(StatusCode::RadioNotConfigured),
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
            StatusCode::RadioNotConfigured => write!(f, "Error: radio is not configured"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RawIq<'a> {
    data: &'a [u8],
}

impl<'a> RawIq<'a> {
    pub fn raw_data(&self) -> &[u8] {
        self.data
    }

    /// returns internal data represented as slice of u16's - as they were measures with the ADC
    pub fn data(&self) -> impl Iterator<Item = u16> + 'a {
        self.data
            .chunks_exact(2)
            .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    pub fn set_data(&mut self, data: &'a [u8]) -> Result<(), Error> {
        if data.len() != crate::RAWIQ_DATA_LENGTH {
            return Err(Error::RawIqDataIncorrectLength);
        }
        self.data = data;
        Ok(())
    }
}

/// Find all possible range in buffer which represent COBS encoded message
pub fn get_possible_cobs_message_ranges<'a>(
    buf: &'a [u8],
) -> impl Iterator<Item = core::ops::Range<usize>> + 'a {
    let mut iter = buf
        .iter()
        .enumerate()
        .filter(|(_, &n)| n == 0x00)
        .map(|(i, _)| i)
        .peekable();

    let mut last_returned_position = 0;
    let mut iter_ends_next_run = false;
    core::iter::from_fn(move || {
        if iter_ends_next_run {
            return None;
        }
        if let Some(position) = iter.next() {
            if let Some(next_position) = iter.peek() {
                if *next_position == position + 1 {
                    iter_ends_next_run = true;
                }
            }
            let range = last_returned_position..position + 1;
            last_returned_position = position + 1;
            return Some(range);
        } else {
            None
        }
    })
}

/// Decodes a buffer containing one or more COBS encoded messages in `buffer` [u8] slice
pub fn try_decode_messages<'a>(
    buffer: &'a mut [u8],
) -> impl Iterator<Item = Result<SerialMessage<'a>, postcard::Error>> + 'a {
    // find [0x00, 0x00] position
    let messages_end = buffer
        .windows(2)
        .position(|w| w == &[0x00, 0x00])
        .unwrap_or(buffer.len());

    let messages = buffer[0..messages_end].split_inclusive_mut(|byte| *byte == 0x00);
    messages.map(|message| postcard::from_bytes_cobs::<SerialMessage>(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_message_ranges() {
        let cobs_encoded_data = [
            0x02, 0x05, 0x00, 0x01, 0x05, 0x03, 0xc0, 0xff, 0xee, 0x00, 0x00, 0x00, 0x00,
        ];
        let message_ranges: Vec<_> =
            get_possible_cobs_message_ranges(&cobs_encoded_data[..]).collect();
        assert_eq!(message_ranges, vec![0..3, 3..10])
    }

    #[test]
    fn test_decode_multiple() {
        let mut cobs_encoded_data = [
            0x02, 0x05, 0x00, 0x01, 0x05, 0x03, 0xc0, 0xff, 0xee, 0x00, 0x00, 0x00, 0x00,
        ];

        let messages: Result<Vec<SerialMessage>, _> =
            try_decode_messages(&mut cobs_encoded_data).collect();
        assert_eq!(
            messages.unwrap(),
            vec![
                SerialMessage::GetRawIq,
                SerialMessage::SendData(SendData {
                    data: &[0xc0, 0xff, 0xee]
                })
            ]
        );
    }

    #[test]
    fn test_message_parse_status() {
        let mut data = [0u8; 256];
        let msg = SerialMessage::new_from_str("status@", &mut data).unwrap();
        assert_eq!(msg, SerialMessage::ReportRequest(ReportRequest));
    }

    #[test]
    fn test_message_parse_send_data() {
        let mut data = [0u8; 256];
        let msg = SerialMessage::new_from_str("send@0xAABB", &mut data).unwrap();
        assert_eq!(
            msg,
            SerialMessage::SendData(SendData {
                data: &[0xaa, 0xbb]
            })
        );

        let msg = SerialMessage::new_from_str("send@0xccdd", &mut data).unwrap();
        assert_eq!(
            msg,
            SerialMessage::SendData(SendData {
                data: &[0xcc, 0xdd]
            })
        );
    }

    #[test]
    fn test_message_parse_config() {
        let mut data = [0u8; 256];
        let msg = SerialMessage::new_from_str("config@1|7|aacc", &mut data).unwrap();
        assert_eq!(
            msg,
            SerialMessage::Configure(Configure {
                region: 1,
                spreading_factor: 7,
                network: [0xaa, 0xcc]
            })
        );
    }

    #[test]
    fn test_message_parse_config_invalid_sf() {
        let mut data = [0u8; 256];
        let err = SerialMessage::new_from_str("config@1|6|aacc", &mut data);
        assert_eq!(err, Err(FromStrError::InvalidMessage));

        let err = SerialMessage::new_from_str("config@1|13|ccaa", &mut data);
        assert_eq!(err, Err(FromStrError::InvalidMessage));
    }

    #[test]
    #[should_panic(expected = "MissingConfigNetwork")]
    fn test_message_parse_config_missing_network() {
        let mut data = [0u8; 256];
        SerialMessage::new_from_str("config@1|12", &mut data).unwrap();
    }

    #[test]
    fn test_message_parse_ts() {
        let mut data = [0u8; 256];
        let msg = SerialMessage::new_from_str("ts@1629896485", &mut data).unwrap();
        assert_eq!(
            msg,
            SerialMessage::SetTimestamp(SetTimestamp {
                timestamp: 1629896485u64
            })
        );
    }
}
