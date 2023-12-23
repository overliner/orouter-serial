//! Serial messages defitions
//!
//! Messages are of two types - depending on direction they are sent. One type goes from host to
//! orouter and the other vice versa.
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
}

#[cfg_attr(feature = "std", derive(Debug, PartialEq))]
pub enum FromStrError {
    MissingSeparator,
    InvalidMessage,
    InvalidHex(base16::DecodeError),
    InvalidHexConfigNetwork,
    InvalidPayloadLength,
    PayloadTooLong,
    MissingConfigNetwork,
    InvalidTargetBuffer,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
#[serde(bound(deserialize = "'de: 'a"))]
pub enum SerialMessage<'a> {
    SendData(SendData<'a>),
    Configure(Configure),
    ReportRequest(ReportRequest),
    UpgradeFirmwareRequest,
    SetTimestamp(SetTimestamp),
    GetRawIq,
    ReceiveData(ReceiveData<'a>),
    Report(Report),
    Status(Status),
    RawIq(RawIq),
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct SendData<'a> {
    /// first byte is length, rest is data
    /// array is always 256 B long,
    data: &'a [u8],
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
    data: &'a [u8],
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
    pub code: crate::host::StatusCode,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct RawIq {
    // FIXME 16_536
    pub data: [u16; 32],
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
}
