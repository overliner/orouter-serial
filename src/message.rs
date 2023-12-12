//! Serial messages defitions
//!
//! Messages are of two types - depending on direction they are sent. One type goes from host to
//! orouter and the other vice versa.
use core::{convert::TryFrom, ops::Range};

use zerocopy::{AsBytes, FromBytes, FromZeroes, Ref};

#[cfg_attr(feature = "std", derive(Debug))]
pub enum Error {
    InvalidMessageType(u8),
    MissingCobsSentinel,
    CantCreatePayloadRef,
    LoraDataTooLong(usize),
    InvalidStatusCode(u8),
    BufferToShortToEncodeMessage,
    CantWriteMessageToBuffer,
    CobsError(corncobs::CobsError),
}

impl From<corncobs::CobsError> for Error {
    fn from(value: corncobs::CobsError) -> Self {
        Error::CobsError(value)
    }
}

#[cfg_attr(feature = "std", derive(Debug))]
pub struct SerialMessage<'a, M: AsBytes + FromBytes + FromZeroes> {
    pub typ: MessageType,
    pub payload: &'a M,
}

impl<'a, M: AsBytes + FromBytes + FromZeroes> SerialMessage<'a, M> {
    /// Creates a SerialMessage instance
    ///
    /// ```rust
    /// use zerocopy::FromZeroes;
    /// use orouter_serial::{host::StatusCode, message::*};
    /// let msg = SerialMessage::new(MessageType::Status, &Status {
    ///     code: StatusCode::FrameReceived as u8
    /// });
    ///
    /// let mut send_data = SendData::new_zeroed();
    /// send_data.put_data(&[0xc0, 0xff, 0xee]);
    /// let msg2 = SerialMessage::new(MessageType::SendData, &send_data);
    /// ```
    pub fn new(typ: MessageType, payload: &'a M) -> Self {
        // FIXME check if payload and typ match
        Self { typ, payload }
    }

    /// Tries to parse SerialMessage from a buffer of bytes
    ///
    /// ```rust
    /// use orouter_serial::message::*;
    /// let mut receive_buffer = [0x03, 0xc5, 0x01, 0x00];
    /// let message = SerialMessage::parse(&mut receive_buffer[..]).unwrap();
    /// assert_eq!(MessageType::Status, message.typ);
    /// let status: &Status = message.payload;
    /// assert_eq!(orouter_serial::host::StatusCode::FrameReceived, status.code().unwrap());
    /// ```
    pub fn parse(bytes: &'a mut [u8]) -> Result<SerialMessage<'a, M>, Error> {
        let len = bytes.len();
        let (typ, decoded_range) = decode_message_type(bytes, 0..len)?;
        let payload = decode_serial_message(&bytes[decoded_range])?;
        Ok(SerialMessage { typ, payload })
    }

    /// Serializes message into a provided buffer and returns iterator with bytes of encoded message
    ///
    /// if `buf` doesn't have needed length for encoding of the mesage it returns error
    pub fn encode_iter(self, buf: &'a mut [u8]) -> Result<impl Iterator<Item = u8> + 'a, Error> {
        encoded_serial_message_as_iter(self.typ.clone(), self.payload, buf)
    }
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum MessageType {
    SendData,
    Configure,
    ReportRequest,
    UpgradeFirmwareRequest,
    SetTimestamp,
    GetRawIq,
    ReceiveData,
    Report,
    Status,
    RawIq,
}

impl TryFrom<u8> for MessageType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0xc0 => Ok(Self::SendData),
            0xc1 => Ok(Self::ReceiveData),
            0xc2 => Ok(Self::Configure),
            0xc3 => Ok(Self::ReportRequest),
            0xc4 => Ok(Self::Report),
            0xc5 => Ok(Self::Status),
            0xc6 => Ok(Self::UpgradeFirmwareRequest),
            0xc7 => Ok(Self::SetTimestamp),
            0xc8 => Ok(Self::GetRawIq),
            0xc9 => Ok(Self::RawIq),
            value => Err(Error::InvalidMessageType(value)),
        }
    }
}

impl Into<u8> for MessageType {
    fn into(self) -> u8 {
        match self {
            Self::SendData => 0xc0,
            Self::ReceiveData => 0xc1,
            Self::Configure => 0xc2,
            Self::ReportRequest => 0xc3,
            Self::Report => 0xc4,
            Self::Status => 0xc5,
            Self::UpgradeFirmwareRequest => 0xc6,
            Self::SetTimestamp => 0xc7,
            Self::GetRawIq => 0xc8,
            Self::RawIq => 0xc9,
        }
    }
}

#[derive(FromZeroes, FromBytes, AsBytes)]
#[repr(C)]
pub struct SendData {
    /// first byte is length, rest is data
    /// array is always 256 B long,
    data: [u8; 256],
}

#[cfg(feature = "std")]
impl core::fmt::Debug for SendData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let used_bytes = self.data[0] as usize;
        write!(
            f,
            "SendData {{ data: {:02x?} }}",
            &self.data[1..used_bytes + 1]
        )
    }
}

impl SendData {
    pub fn put_data(&mut self, data: &[u8]) -> Result<(), Error> {
        // TODO lora const
        if data.len() > 255 {
            return Err(Error::LoraDataTooLong(data.len()));
        }
        self.data[0] = data.len() as u8;
        self.data[1..data.len() + 1].copy_from_slice(data);
        Ok(())
    }

    pub fn data(&self) -> &[u8] {
        let len = (self.data[0] + 1) as usize;
        &self.data[1..len]
    }
}

#[derive(FromZeroes, FromBytes, AsBytes)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct Configure {
    pub region: u8,
    pub spreading_factor: u8,
    pub network: [u8; 2],
}

#[derive(FromZeroes, FromBytes, AsBytes, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct ReportRequest;

#[derive(FromZeroes, FromBytes, AsBytes)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct UpgradeFirmwareRequest;

#[derive(FromZeroes, FromBytes, AsBytes)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct SetTimestamp {
    pub timestamp: u64,
}

#[derive(FromZeroes, FromBytes, AsBytes)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct GetRawIq;

#[derive(FromZeroes, FromBytes, AsBytes)]
#[repr(C)]
pub struct ReceiveData {
    /// first byte is length, rest is data
    /// array is always 256 B long,
    data: [u8; 256],
}

#[cfg(feature = "std")]
impl core::fmt::Debug for ReceiveData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let used_bytes = self.data[0] as usize;
        write!(
            f,
            "ReceiveData {{ data: {:02x?} }}",
            &self.data[1..used_bytes + 1]
        )
    }
}

impl ReceiveData {
    pub fn put_data(&mut self, data: &[u8]) -> Result<(), Error> {
        // TODO lora const
        if data.len() > 255 {
            return Err(Error::LoraDataTooLong(data.len()));
        }
        self.data[0] = data.len() as u8;
        self.data[1..data.len() + 1].copy_from_slice(data);
        Ok(())
    }

    pub fn data(&self) -> &[u8] {
        let len = (self.data[0] + 1) as usize;
        &self.data[1..len]
    }
}

#[derive(FromZeroes, FromBytes, AsBytes)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct Report {
    pub sn: u32,
    pub version_data: [u8; 9],
    pub region: u8,
    pub spreading_factor: u8,
    pub receive_queue_size: u8,
    pub transmit_queue_size: u8,
    pub network: [u8; 2],
    __unused_padding: [u8; 1],
}

#[derive(FromBytes, FromZeroes, AsBytes, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct Status {
    pub code: u8,
}

impl Status {
    pub fn code(&self) -> Result<crate::host::StatusCode, Error> {
        crate::host::StatusCode::try_from(self.code)
            .map_err(|_| Error::InvalidStatusCode(self.code))
    }
}

#[derive(FromZeroes, FromBytes, AsBytes)]
#[cfg_attr(feature = "std", derive(Debug))]
#[repr(C)]
pub struct RawIq {
    pub data: [u8; 16_536],
}

/// Decode a MessageType from buffer in slice of range_encoded returns decoded message type and new
/// range in provided slice where the decoded message payload is newly - this is because cobs
/// decoding is happenning in place to save memory
pub(crate) fn decode_message_type(
    buf: &mut [u8],
    range_encoded: Range<usize>,
) -> Result<(MessageType, Range<usize>), Error> {
    let _ = buf
        .iter()
        .position(|&i| i == 0x00)
        .ok_or(Error::MissingCobsSentinel)?;

    let original_start = range_encoded.start.clone();
    let decoded_len = corncobs::decode_in_place(&mut buf[range_encoded])?;
    let typ = MessageType::try_from(buf[original_start])?;

    Ok((typ, original_start + 1..original_start + decoded_len))
}

fn decode_serial_message<M: FromBytes>(payload: &[u8]) -> Result<&M, Error> {
    Ref::<_, M>::new(payload)
        .ok_or(Error::CantCreatePayloadRef)
        .map(|r| r.into_ref())
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

fn encoded_serial_message_as_iter<'a, M: AsBytes>(
    typ: MessageType,
    serial_message: &M,
    buf: &'a mut [u8],
) -> Result<impl Iterator<Item = u8> + 'a, Error> {
    buf[0] = typ.into();
    serial_message
        .write_to(&mut buf[1..1 + core::mem::size_of::<M>()])
        .ok_or(Error::CantWriteMessageToBuffer)?;
    Ok(corncobs::encode_iter(
        &buf[0..1 + core::mem::size_of::<M>()],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_usage() {
        let mut buf = [0u8; 257];
        let sd: &mut SendData = SendData::mut_from_prefix(&mut buf[..256]).unwrap();
        sd.put_data(&[0xc0, 0xff, 0xee]).unwrap();

        // sd.write_to_prefix(&mut buf[..]);
        assert_eq!(&buf[0..4], &[0x03, 0xc0, 0xff, 0xee]);

        let sd = SendData::ref_from_prefix(&buf[..]).unwrap();
        assert_eq!(&sd.data(), &[0xc0, 0xff, 0xee]);

        let mut buf = [0u8; 257];
        let rr = ReportRequest::new_zeroed();
        rr.write_to(&mut buf[0..0]).ok_or(()).unwrap();
    }

    #[test]
    fn test_decode_send_data() {
        let mut buf = [0x00; 256];
        buf[0..5].copy_from_slice(&[0x04, 0xde, 0xad, 0xbe, 0xef]);
        let msg: &SendData = decode_serial_message(&buf[..]).unwrap();
        assert_eq!(msg.data(), &[0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn test_decode_multiple_messages() {
        let mut buf = [0x02, 0xc3, 0x00, 0x03, 0xc5, 0x01, 0x00];
        let valid_ranges = [0..3, 3..7];

        for (i, range) in valid_ranges.into_iter().enumerate() {
            let (typ, decoded_range) = decode_message_type(&mut buf[..], range).unwrap();
            match i {
                0 => {
                    assert_eq!(typ, MessageType::ReportRequest);
                    let msg: &ReportRequest = decode_serial_message(&buf[decoded_range]).unwrap();
                    assert_eq!(msg, &ReportRequest::new_zeroed());
                }
                1 => {
                    assert_eq!(typ, MessageType::Status);
                    let msg: &Status = decode_serial_message(&buf[decoded_range]).unwrap();
                    assert_eq!(msg, &Status { code: 1 });
                }
                _ => assert!(false, "unknown case"),
            }
        }
    }

    #[test]
    fn test_get_positions() {
        let buf = [
            0x02, 0xc3, 0x00, 0x03, 0xc5, 0x01, 0x00, 0x02, 0xc6, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut iter = get_possible_cobs_message_ranges(&buf);
        assert_eq!(iter.next(), Some(0..3)); // 0x02, 0xc3, 0x00
        assert_eq!(iter.next(), Some(3..7)); // 0x03, 0xc5, 0x01, 0x00
        assert_eq!(iter.next(), Some(7..10)); // 0x02, 0xc6, 0x00

        // for range in get_possible_cobs_message_ranges(&buf) {
        //     println!(
        //         "TT: range = {:?}..{:?} msg = {:02x?}",
        //         range.start.clone(),
        //         range.end.clone(),
        //         &buf[range]
        //     );
        // }
        // assert!(false);
    }

    #[test]
    fn test_roundtrip_decode_encode() {
        const ORIGINAL_DATA: [u8; 13] = [
            0x02, 0xc3, 0x00, 0x03, 0xc5, 0x01, 0x00, 0x02, 0xc6, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut buf = ORIGINAL_DATA.clone();
        let mut encode_buf = [0u8; ORIGINAL_DATA.len()];
        let mut output_buf = [0u8; ORIGINAL_DATA.len()];
        let mut last_output_index = 0;
        let ranges = get_possible_cobs_message_ranges(&buf).collect::<Vec<Range<usize>>>();
        for range in ranges {
            let (typ, decoded_range) = decode_message_type(&mut buf[..], range.clone()).unwrap();
            match typ {
                MessageType::ReportRequest => {
                    let msg: &ReportRequest =
                        decode_serial_message(&buf[decoded_range.clone()]).unwrap();
                    let encoded_bytes =
                        encoded_serial_message_as_iter(typ, msg, &mut encode_buf[range.clone()])
                            .unwrap()
                            .collect::<Vec<u8>>();

                    output_buf[last_output_index..last_output_index + encoded_bytes.len()]
                        .copy_from_slice(encoded_bytes.as_slice());
                    last_output_index += encoded_bytes.len();
                }
                MessageType::Status => {
                    let msg: &Status = decode_serial_message(&buf[decoded_range.clone()]).unwrap();
                    let encoded_bytes =
                        encoded_serial_message_as_iter(typ, msg, &mut encode_buf[range.clone()])
                            .unwrap()
                            .collect::<Vec<u8>>();

                    output_buf[last_output_index..last_output_index + encoded_bytes.len()]
                        .copy_from_slice(encoded_bytes.as_slice());
                    last_output_index += encoded_bytes.len();
                }
                MessageType::UpgradeFirmwareRequest => {
                    let msg: &UpgradeFirmwareRequest =
                        decode_serial_message(&buf[decoded_range.clone()]).unwrap();
                    let encoded_bytes =
                        encoded_serial_message_as_iter(typ, msg, &mut encode_buf[range.clone()])
                            .unwrap()
                            .collect::<Vec<u8>>();

                    output_buf[last_output_index..last_output_index + encoded_bytes.len()]
                        .copy_from_slice(encoded_bytes.as_slice());
                    last_output_index += encoded_bytes.len();
                }
                _ => assert!(false, "unexpected message type"),
            }
        }
        assert_eq!(
            ORIGINAL_DATA, output_buf,
            "\nleft  = {:02x?}\nright = {:02x?}",
            ORIGINAL_DATA, output_buf
        );
    }

    #[test]
    pub fn test_serial_message_parse_send_data() {
        // manually create COBS serialized a SendData { data: [0xc0, 0xff, 0xee] } message:
        let mut buf = [1u8; 259];
        buf[0..6].copy_from_slice(&[0x06, 0xc0, 0x03, 0xc0, 0xff, 0xee]);
        buf[258] = 0x00;

        let msg = SerialMessage::parse(&mut buf[..]).unwrap();
        match msg.typ {
            MessageType::SendData => {
                let data: &SendData = msg.payload;
                assert_eq!(data.data(), &[0xc0, 0xff, 0xee]);
            }
            _ => assert!(false, "invalid variant, expected SendData, got {msg:?}"),
        }
    }

    #[test]
    pub fn test_serial_message_parse_status() {
        let mut buf = [0u8; 20];
        buf[0..4].copy_from_slice(&[0x03, 0xc5, 0x02, 0x00]);
        let msg = SerialMessage::parse(&mut buf[0..4]).unwrap();
        match msg.typ {
            MessageType::Status => {
                let status: &Status = msg.payload;
                assert_eq!(
                    status.code().unwrap(),
                    crate::host::StatusCode::CommandReceived
                )
            }
            _ => assert!(false, "invalid variant, expected Status, got {msg:?}"),
        }
    }

    #[test]
    pub fn test_serial_message_status_encode_iter() {
        let mut output_buf = [0u8, 10];
        let status = Status {
            code: crate::host::StatusCode::FrameReceived as u8,
        };
        let msg = SerialMessage::new(MessageType::Status, &status);

        let serialized = msg
            .encode_iter(&mut output_buf)
            .unwrap()
            .collect::<Vec<u8>>();

        assert_eq!(serialized.as_slice(), &[0x03, 0xc5, 0x01, 0x00]);
    }
}
