use heapless::Vec;

const MAX_BLE_FRAME_LENGTH: usize = 128; // BLE can only process this
pub const MAX_USB_FRAME_LENGTH: usize = 256; // USB can only process this
///
/// cannot run calculation in const declaration
/// calculation is min(1, MAX_MESSAGE_LENGTH_HEX_ENCODED % MAX_SERIAL_FRAME_LENGTH);
/// which is min(1, 520 % 128) = min(1, 8) = 1
const MAX_BLE_FRAMES_COUNT_REMAINDER: usize = 1;

/// cannot run calculation in const declaration
/// calculation is min(1, MAX_MESSAGE_LENGTH_HEX_ENCODED % MAX_SERIAL_FRAME_LENGTH);
/// which is min(1, 260 % 256) = min(1, 1) = 1
///
const MAX_USB_FRAMES_COUNT_REMAINDER: usize = 1;

pub const MAX_USB_FRAMES_COUNT: usize =
    super::MAX_MESSAGE_LENGTH / MAX_USB_FRAME_LENGTH + MAX_USB_FRAMES_COUNT_REMAINDER;

pub const MAX_BLE_FRAMES_COUNT: usize =
    MAX_MESSAGE_LENGTH_HEX_ENCODED / MAX_BLE_FRAME_LENGTH + MAX_BLE_FRAMES_COUNT_REMAINDER;

pub const MAX_MESSAGE_LENGTH_HEX_ENCODED: usize = 2 * super::MAX_MESSAGE_LENGTH; // hex encoding - each byte = 2 chars

const MAX_BGX13_MESSAGE_LENGTH: usize = 32;
const MAX_BGX13_FRAMES_COUNT: usize = 9;

const MAX_BT4502_MESSAGE_LENGTH: usize = 240;
const MAX_BT4502_FRAMES_COUNT: usize = 3;

type UsbSerialFrameVec = Vec<u8, MAX_USB_FRAME_LENGTH>;
type BleSerialFrameVec = Vec<u8, MAX_BLE_FRAME_LENGTH>;
type Bgx13SerialFrameVec = Vec<u8, MAX_BGX13_MESSAGE_LENGTH>;
type Bt4502SerialFrameVec = Vec<u8, MAX_BT4502_MESSAGE_LENGTH>;

pub trait WireCodec {
    const MESSAGE_DELIMITER: Option<char>;
    type Frames;
    type IncomingFrame: IntoIterator<Item = u8> + AsRef<[u8]>;

    fn get_frames(data: &mut [u8]) -> Result<Self::Frames, CodecError>;
    fn decode_frame(data: &[u8]) -> Result<(Self::IncomingFrame, usize), CodecError>;
}

#[derive(PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum CodecError {
    FrameCreateError,
    DecodeError,
    MalformedHex(base16::DecodeError),
}

impl From<base16::DecodeError> for CodecError {
    fn from(e: base16::DecodeError) -> CodecError {
        CodecError::MalformedHex(e)
    }
}

pub struct Rn4870Codec {}

impl WireCodec for Rn4870Codec {
    const MESSAGE_DELIMITER: Option<char> = Some('%');
    type Frames = Vec<BleSerialFrameVec, MAX_BLE_FRAMES_COUNT>;
    type IncomingFrame = BleSerialFrameVec;

    fn get_frames(data: &mut [u8]) -> Result<Self::Frames, CodecError> {
        let mut hex_result = Vec::<u8, MAX_MESSAGE_LENGTH_HEX_ENCODED>::new();
        hex_result
            .resize_default(data.len() * 2)
            .map_err(|_| CodecError::FrameCreateError)?;
        base16::encode_config_slice(&data, base16::EncodeLower, &mut hex_result);

        // wrap each chunk in a delimiter char
        let mut frames = Vec::<BleSerialFrameVec, MAX_BLE_FRAMES_COUNT>::new();
        for chunk in hex_result.chunks_mut(MAX_BLE_FRAME_LENGTH - 2) {
            let mut frame = BleSerialFrameVec::new();
            if let Some(delim) = Self::MESSAGE_DELIMITER {
                frame
                    .push(delim as u8)
                    .map_err(|_| CodecError::FrameCreateError)?;
            };
            frame
                .extend_from_slice(&chunk)
                .map_err(|_| CodecError::FrameCreateError)?;
            if let Some(delim) = Self::MESSAGE_DELIMITER {
                frame
                    .push(delim as u8)
                    .map_err(|_| CodecError::FrameCreateError)?;
            };
            frames
                .push(frame)
                .map_err(|_| CodecError::FrameCreateError)?
        }
        Ok(frames)
    }

    fn decode_frame(data: &[u8]) -> Result<(Self::IncomingFrame, usize), CodecError> {
        let mut decoded = Vec::<u8, 64>::new();
        decoded
            .resize_default(64)
            .map_err(|_| CodecError::DecodeError)?;
        match base16::decode_slice(&data, &mut decoded) {
            Ok(decoded_len) => {
                let result = Vec::from_slice(&decoded[0..decoded_len])
                    .map_err(|_| CodecError::DecodeError)?;
                Ok((result, decoded_len))
            }
            Err(e) => Err(CodecError::MalformedHex(e)),
        }
    }
}

pub struct UsbCodec {}

impl WireCodec for UsbCodec {
    type Frames = Vec<UsbSerialFrameVec, MAX_USB_FRAMES_COUNT>;
    type IncomingFrame = UsbSerialFrameVec;
    const MESSAGE_DELIMITER: Option<char> = None;

    fn get_frames(data: &mut [u8]) -> Result<Self::Frames, CodecError> {
        let mut frames = Vec::<UsbSerialFrameVec, MAX_USB_FRAMES_COUNT>::new();
        for chunk in data.chunks_mut(MAX_USB_FRAME_LENGTH) {
            frames
                .push(
                    UsbSerialFrameVec::from_slice(&chunk)
                        .map_err(|_| CodecError::FrameCreateError)?,
                )
                .map_err(|_| CodecError::FrameCreateError)?
        }
        Ok(frames)
    }
    fn decode_frame(data: &[u8]) -> Result<(Self::IncomingFrame, usize), CodecError> {
        let mut decoded = UsbSerialFrameVec::new();
        decoded
            .extend_from_slice(data)
            .map_err(|_| CodecError::DecodeError)?;
        Ok((decoded, data.len()))
    }
}

pub struct Bgx13Codec {}

impl WireCodec for Bgx13Codec {
    type Frames = Vec<Bgx13SerialFrameVec, MAX_BGX13_FRAMES_COUNT>;
    type IncomingFrame = Bgx13SerialFrameVec;
    const MESSAGE_DELIMITER: Option<char> = None;

    fn get_frames(data: &mut [u8]) -> Result<Self::Frames, CodecError> {
        let mut frames = Vec::<Bgx13SerialFrameVec, MAX_BGX13_FRAMES_COUNT>::new();
        for chunk in data.chunks_mut(MAX_BGX13_MESSAGE_LENGTH) {
            frames
                .push(
                    Bgx13SerialFrameVec::from_slice(&chunk)
                        .map_err(|_| CodecError::FrameCreateError)?,
                )
                .map_err(|_| CodecError::FrameCreateError)?
        }
        Ok(frames)
    }
    fn decode_frame(data: &[u8]) -> Result<(Self::IncomingFrame, usize), CodecError> {
        let mut decoded = Bgx13SerialFrameVec::new();
        decoded
            .extend_from_slice(data)
            .map_err(|_| CodecError::DecodeError)?;
        Ok((decoded, data.len()))
    }
}

pub struct Bt4502Codec {}

impl WireCodec for Bt4502Codec {
    type Frames = Vec<Bt4502SerialFrameVec, MAX_BT4502_FRAMES_COUNT>;
    type IncomingFrame = Bt4502SerialFrameVec;
    const MESSAGE_DELIMITER: Option<char> = None;

    fn get_frames(data: &mut [u8]) -> Result<Self::Frames, CodecError> {
        let mut frames = Vec::<Bt4502SerialFrameVec, MAX_BT4502_FRAMES_COUNT>::new();
        for chunk in data.chunks_mut(MAX_BT4502_MESSAGE_LENGTH) {
            match chunk[0..4] {
                [b'T', b'T', b'M', b':'] => {
                    frames
                        .push(
                            Bt4502SerialFrameVec::from_slice(&chunk[0..2])
                                .map_err(|_| CodecError::FrameCreateError)?,
                        )
                        .map_err(|_| CodecError::FrameCreateError)?;
                    frames
                        .push(
                            Bt4502SerialFrameVec::from_slice(&chunk[2..])
                                .map_err(|_| CodecError::FrameCreateError)?,
                        )
                        .map_err(|_| CodecError::FrameCreateError)?;
                }
                _ => frames
                    .push(
                        Bt4502SerialFrameVec::from_slice(&chunk)
                            .map_err(|_| CodecError::FrameCreateError)?,
                    )
                    .map_err(|_| CodecError::FrameCreateError)?,
            };
        }
        Ok(frames)
    }

    fn decode_frame(data: &[u8]) -> Result<(Self::IncomingFrame, usize), CodecError> {
        let mut decoded = Bt4502SerialFrameVec::new();
        decoded
            .extend_from_slice(data)
            .map_err(|_| CodecError::DecodeError)?;
        Ok((decoded, data.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_max_usb_frames() {
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
        let msg = super::super::Message::SendData {
            data: Vec::<u8, 255>::from_slice(&encoded[..]).unwrap(),
        };

        let frames = msg.as_frames::<UsbCodec>().unwrap();

        assert_eq!(frames.len(), 2);

        let result = &frames[0];
        let last_frame = &frames.last().unwrap();
        assert_eq!(result.len(), MAX_USB_FRAME_LENGTH);
        for b in &result[0..result.len() - 2] {
            assert_ne!(0x00, *b);
        }
        assert_eq!(Some(&0x00), last_frame.last());
    }
    #[test]
    fn test_bt4502() {
        let frames = Bt4502Codec::get_frames(&mut [0x01, 0x02, 0x03, 0x04]).unwrap();
        assert_eq!(frames.len(), 1);
    }
    #[test]
    fn test_bt4502_split() {
        let frames = Bt4502Codec::get_frames(&mut [b'T', b'T', b'M', b':']).unwrap();
        assert_eq!(frames[0].len(), 2);
        assert_eq!(frames[1].len(), 2);
        assert_eq!(frames.len(), 2);
    }
}
