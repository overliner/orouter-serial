use heapless::{consts::*, Vec};
use typenum::Unsigned;

pub type MaxLoraPayloadLength = U255;
pub type OverlineMessageHashLength = U16;
pub type OverlineMessageData = Vec<u8, MaxLoraPayloadLength>;
pub type OverlineMessageHash = Vec<u8, OverlineMessageHashLength>;

#[derive(Debug, PartialEq)]
pub enum Error {
    InvalidOverlineMessage,
    UnknownType,
}

// FIXME define these according to the design document
#[derive(Debug, PartialEq)]
pub enum OverlineMessageType {
    Challenge,
    Proof,
    Flush,
    Receipt,
    Other,
}

/// Logical message of overline protocol - does not contain any link level data
/// (e.g. magic byte, message type, or information about how 512B message was transferred)
#[derive(Debug)]
pub struct OverlineMessage(pub OverlineMessageData);

impl OverlineMessage {
    pub fn hash(&self) -> Result<OverlineMessageHash, Error> {
        if self.0.len() < OverlineMessageHashLength::USIZE {
            return Err(Error::InvalidOverlineMessage);
        }

        match OverlineMessageHash::from_slice(&self.0[0..OverlineMessageHashLength::USIZE]) {
            Ok(h) => Ok(h),
            Err(()) => Err(Error::InvalidOverlineMessage),
        }
    }

    pub fn typ(&self) -> Result<OverlineMessageType, Error> {
        match self.0[OverlineMessageHashLength::USIZE] {
            0x11 => Ok(OverlineMessageType::Challenge),
            0x12 => Ok(OverlineMessageType::Proof),
            0x13 => Ok(OverlineMessageType::Flush),
            0x14 => Ok(OverlineMessageType::Receipt),
            0x15 => Ok(OverlineMessageType::Other),
            _ => Err(Error::UnknownType),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_ok() {
        let m = OverlineMessage(
            OverlineMessageData::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc, 0xff, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(
            OverlineMessageHash::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc
            ])
            .unwrap(),
            m.hash().unwrap()
        )
    }

    #[test]
    fn test_typ_err() {
        let m = OverlineMessage(
            OverlineMessageData::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc, 0xff, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(Err(Error::UnknownType), m.typ())
    }

    #[test]
    fn test_typ_ok() {
        let m = OverlineMessage(
            OverlineMessageData::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc, 0x11, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(OverlineMessageType::Challenge, m.typ().unwrap())
    }
}
