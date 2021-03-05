//! Defines Overline protocol
//!
//! Describes types and structure of logical overline message - how it is represented in the
//! physical LoRa message. Defines utility struct [MessageStore] which enabled
//! implementation of overline message retransmission rules
use heapless::{consts::*, FnvIndexMap, FnvIndexSet, Vec};
use typenum::{op, Unsigned, *};

pub type MaxLoraPayloadLength = U255;
pub type MessageHashLength = U16;
pub type MessageMaxDataLength = op!(MaxLoraPayloadLength - MessageHashLength);
pub type MessageDataPart = Vec<u8, MessageMaxDataLength>; // FIXME better naming
pub type MessageHash = Vec<u8, MessageHashLength>;

#[derive(Debug, PartialEq)]
pub enum Error {
    InvalidMessage,
    UnknownType,
}

// FIXME define these according to the design document
#[derive(Debug, PartialEq)]
pub enum MessageType {
    Challenge,
    Proof,
    Flush,
    Receipt,
    Other,
}

/// Logical message of overline protocol - does not contain any link level data
/// (e.g. magic byte, message type, or information about how 512B message was transferred)
#[derive(Debug)]
pub struct Message(Vec<u8, MaxLoraPayloadLength>);

impl Message {
    pub fn try_from_hash_data(hash: MessageHash, data: MessageDataPart) -> Result<Self, Error> {
        if hash.len() != MessageHashLength::USIZE {
            return Err(Error::InvalidMessage);
        }

        if hash.len() + data.len() > MaxLoraPayloadLength::USIZE {
            return Err(Error::InvalidMessage);
        }

        let mut vec = Vec::new();
        vec.extend_from_slice(&hash[0..])
            .map_err(|_| Error::InvalidMessage)?;
        vec.extend_from_slice(&data[0..])
            .map_err(|_| Error::InvalidMessage)?;

        Ok(Message(vec))
    }

    pub fn into_hash_data(self) -> Result<(MessageHash, MessageDataPart), Error> {
        let hash = self.hash()?;
        let data_part = self.data_part()?;
        Ok((hash, data_part))
    }

    pub fn hash(&self) -> Result<MessageHash, Error> {
        if self.0.len() < MessageHashLength::USIZE {
            return Err(Error::InvalidMessage);
        }

        match MessageHash::from_slice(&self.0[0..MessageHashLength::USIZE]) {
            Ok(h) => Ok(h),
            Err(()) => Err(Error::InvalidMessage),
        }
    }

    pub fn data_part(&self) -> Result<MessageDataPart, Error> {
        match MessageDataPart::from_slice(&self.0[MessageHashLength::USIZE..]) {
            Ok(h) => Ok(h),
            Err(()) => Err(Error::InvalidMessage),
        }
    }

    pub fn typ(&self) -> Result<MessageType, Error> {
        match self.0[MessageHashLength::USIZE] {
            0x11 => Ok(MessageType::Challenge),
            0x12 => Ok(MessageType::Proof),
            0x13 => Ok(MessageType::Flush),
            0x14 => Ok(MessageType::Receipt),
            0x15 => Ok(MessageType::Other),
            _ => Err(Error::UnknownType),
        }
    }
}

/// Describes outcome of attempt to [`MessageStore::recv`]
pub enum StoreRecvOutcome {
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
pub struct MessageStore {
    /// one tick duration in ms, used for deciding expiration in [`Self::tick_try_send`]
    tick_duration: u32,
    short_term_queue: FnvIndexMap<MessageHash, Message, U256>,
    long_term_queue: FnvIndexSet<MessageHash, U1024>,
}

impl MessageStore {
    pub fn new() -> Self {
        MessageStore::default()
    }

    /// used when node received a message
    pub fn recv(&mut self, message: Message) -> Result<StoreRecvOutcome, ()> {
        let hash = message.hash().unwrap();
        // if we have seen this, immediately remove it from short term queue and store in long term queue
        if self.short_term_queue.contains_key(&hash) {
            todo!();
            return Ok(StoreRecvOutcome::Seen);
        }

        // if not, store hash and body and enqueue to short term queue with a random timeout
        self.short_term_queue
            .insert(message.hash().unwrap(), message)
            .unwrap();

        Ok(StoreRecvOutcome::Todo)
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
        let m = Message(
            Vec::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc, 0xff, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(
            MessageHash::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc
            ])
            .unwrap(),
            m.hash().unwrap()
        )
    }

    #[test]
    fn test_typ_err() {
        let m = Message(
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
        let m = Message(
            Vec::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                0xbb, 0xcc, 0x11, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(MessageType::Challenge, m.typ().unwrap())
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

        let m = Message::try_from_hash_data(hash.clone(), data.clone()).unwrap();
        assert_eq!(hash, m.hash().unwrap());
        assert_eq!(MessageType::Other, m.typ().unwrap());

        // m moves here
        let (hash_new, data_new) = m.into_hash_data().unwrap();
        assert_eq!(hash, hash_new);
        assert_eq!(data, data_new);
    }
}
