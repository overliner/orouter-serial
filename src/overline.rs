//! Defines Overline protocol
//!
//! Describes types and structure of logical overline message - how it is represented in the
//! physical LoRa message. Defines utility struct [OverlineMessageStore] which enabled
//! implementation of overline message retransmission rules
use heapless::{consts::*, FnvIndexMap, FnvIndexSet, Vec};
use typenum::{op, Unsigned, *};

pub type MaxLoraPayloadLength = U255;
pub type OverlineMessageHashLength = U16;
pub type OverlineMessageMaxDataLength = op!(MaxLoraPayloadLength - OverlineMessageHashLength);
pub type OverlineMessageDataPart = Vec<u8, OverlineMessageMaxDataLength>; // FIXME better naming
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
pub struct OverlineMessage(Vec<u8, MaxLoraPayloadLength>);

impl OverlineMessage {
    pub fn try_from_hash_data(
        hash: OverlineMessageHash,
        data: OverlineMessageDataPart,
    ) -> Result<Self, Error> {
        if hash.len() != OverlineMessageHashLength::USIZE {
            return Err(Error::InvalidOverlineMessage);
        }

        if hash.len() + data.len() > MaxLoraPayloadLength::USIZE {
            return Err(Error::InvalidOverlineMessage);
        }

        let mut vec = Vec::new();
        vec.extend_from_slice(&hash[0..])
            .map_err(|_| Error::InvalidOverlineMessage)?;
        vec.extend_from_slice(&data[0..])
            .map_err(|_| Error::InvalidOverlineMessage)?;

        Ok(OverlineMessage(vec))
    }

    pub fn into_hash_data(self) -> Result<(OverlineMessageHash, OverlineMessageDataPart), Error> {
        let hash = self.hash()?;
        let data_part = self.data_part()?;
        Ok((hash, data_part))
    }

    pub fn hash(&self) -> Result<OverlineMessageHash, Error> {
        if self.0.len() < OverlineMessageHashLength::USIZE {
            return Err(Error::InvalidOverlineMessage);
        }

        match OverlineMessageHash::from_slice(&self.0[0..OverlineMessageHashLength::USIZE]) {
            Ok(h) => Ok(h),
            Err(()) => Err(Error::InvalidOverlineMessage),
        }
    }

    pub fn data_part(&self) -> Result<OverlineMessageDataPart, Error> {
        match OverlineMessageDataPart::from_slice(&self.0[OverlineMessageHashLength::USIZE..]) {
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

/// Describes outcome of attempt to [`OverlineMessageStore::recv`]
pub enum OverlineStoreRecvOutcome {
    /// hash was not in the short term queue, scheduled for retransmission
    NotSeenScheduled(u8),
    /// message was seen, removed from short term queue
    Seen,
    /// message was a command
    Command,
    Todo, // TODO remove, used for blank implementation
}

/// Store is responsible for applying rules for storing and possible retransmission of overline
/// messages seen by the node
#[derive(Default)]
pub struct OverlineMessageStore {
    /// one tick duration in ms, used for deciding expiration in [`Self::tick_try_send`]
    tick_duration: u32,
    short_term_queue: FnvIndexMap<OverlineMessageHash, OverlineMessage, U256>,
    long_term_queue: FnvIndexSet<OverlineMessageHash, U1024>,
}

impl OverlineMessageStore {
    pub fn new() -> Self {
        OverlineMessageStore::default()
    }

    /// used when node received a message
    pub fn recv(&mut self, message: OverlineMessage) -> Result<OverlineStoreRecvOutcome, ()> {
        let hash = message.hash().unwrap();
        // if we have seen this, immediately remove it from short term queue and store in long term queue
        if self.short_term_queue.contains_key(&hash) {
            todo!();
            return Ok(OverlineStoreRecvOutcome::Seen);
        }

        // if not, store hash and body and enqueue to short term queue with a random timeout
        self.short_term_queue
            .insert(message.hash().unwrap(), message)
            .unwrap();

        Ok(OverlineStoreRecvOutcome::Todo)
    }

    /// supposed to be driven by a timer, if current tick >= some of the scheduled ticks in the
    /// short term queue it means, the message was not seen during the timeout interval and should
    /// be scheduled for retransmission into the tx queue
    pub fn tick_try_send(&mut self) -> Result<(), ()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_ok() {
        let m = OverlineMessage(
            Vec::from_slice(&[
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
            Vec::from_slice(&[
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
            Vec::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc, 0x11, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(OverlineMessageType::Challenge, m.typ().unwrap())
    }

    #[test]
    fn test_try_from_into_hash_data() {
        let hash = Vec::<u8, U16>::from_slice(&[
            // hash
            0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10,
            0xaa, 0x10,
        ])
        .unwrap();
        let data = Vec::<u8, U239>::from_slice(&[
            // type (other) + some data ->
            0x15, 0xda, 0x1a, 0xda, 0x1a,
        ])
        .unwrap();

        let m = OverlineMessage::try_from_hash_data(hash.clone(), data.clone()).unwrap();
        assert_eq!(hash, m.hash().unwrap());
        assert_eq!(OverlineMessageType::Other, m.typ().unwrap());

        // m moves here
        let (hash_new, data_new) = m.into_hash_data().unwrap();
        assert_eq!(hash, hash_new);
        assert_eq!(data, data_new);
    }
}
