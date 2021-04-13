//! Defines Overline protocol
//!
//! Describes types and structure of logical overline message - how it is represented in the
//! physical LoRa message. Defines utility struct [MessageStore] which enabled
//! implementation of overline message retransmission rules
use core::cmp::{Ord, Ordering};

use heapless::{consts::*, FnvIndexSet, Vec};
use rand::prelude::*;
use typenum::{op, Unsigned};

pub type MaxLoraPayloadLength = U255;
pub type MessageHashLength = U16;
pub type MessageMaxDataLength = op!(MaxLoraPayloadLength - MessageHashLength);
pub type MessageDataPart = Vec<u8, MessageMaxDataLength>; // FIXME better naming
pub type MessageHash = Vec<u8, MessageHashLength>;

#[derive(Debug, PartialEq)]
pub enum Error {
    InvalidMessage,
    CannotReceive,
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
#[derive(Debug, Clone, PartialEq)]
pub struct Message(Vec<u8, MaxLoraPayloadLength>);

// FIXME implement into host::Message::SendData and host::Message::ReceiveData
impl Message {
    pub fn new(data: Vec<u8, MaxLoraPayloadLength>) -> Self {
        Message(data)
    }

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

    pub fn as_vec(self) -> Vec<u8, MaxLoraPayloadLength> {
        self.0
    }
}

/// Describes outcome of attempt to [`MessageStore::recv`]
#[derive(Debug, PartialEq)]
pub enum StoreRecvOutcome {
    /// hash was not in the short term queue, scheduled for retransmission
    NotSeenScheduled(u16),
    /// message was seen, removed from short term queue
    Seen,
    /// message was a command
    Command,
}

/// Wraps message parts (hash and data) for storing them ordered in the short term queue
#[derive(Eq, Debug)]
pub struct ShortTermQueueItem {
    /// determines in how many ticks the item expires for the short term queue
    when: u16,
    message_hash: MessageHash,
    message_data_part: MessageDataPart,
}

impl Ord for ShortTermQueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.when.cmp(&other.when)
    }
}

impl PartialOrd for ShortTermQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ShortTermQueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.when == other.when
    }
}

#[cfg(feature = "debug")]
type ShortTermQueueLength = U16;
#[cfg(not(feature = "debug"))]
type ShortTermQueueLength = U64;

/// Store is responsible for applying rules for storing and possible retransmission of overline
/// messages seen by the node
#[derive(Debug)]
pub struct MessageStore<R: RngCore> {
    /// one tick duration in ms, used for deciding expiration in [`Self::tick_try_send`]
    tick_duration: u16,
    tick_count: u16,
    // short_term_queue: FnvIndexMap<MessageHash, MessageDataPart, ShortTermQueueLength>,
    short_term_queue: Vec<Option<ShortTermQueueItem>, ShortTermQueueLength>,
    #[cfg(feature = "debug")]
    long_term_queue: FnvIndexSet<MessageHash, U64>,
    #[cfg(not(feature = "debug"))]
    long_term_queue: FnvIndexSet<MessageHash, U512>,
    rng: R,
}

impl<R: RngCore> MessageStore<R> {
    pub fn new(rng: R) -> Self {
        MessageStore {
            tick_duration: Default::default(),
            tick_count: Default::default(),
            short_term_queue: Default::default(),
            long_term_queue: Default::default(),
            rng,
        }
    }

    /// used when node received a message
    pub fn recv(&mut self, message: Message) -> Result<StoreRecvOutcome, Error> {
        let (message_hash, message_data_part) = message.into_hash_data()?;
        // if we have seen this, and is in short term queue, immediately remove it from short term queue and store in long term queue
        let mut idx: usize = 0;
        let mut remove_idx = None;
        for item in &self.short_term_queue {
            if let Some(item) = item {
                if item.message_hash == message_hash {
                    remove_idx = Some(idx);
                    break;
                }
            }
            idx += 1;
        }

        if let Some(remove_idx) = remove_idx {
            // TODO solve short term queue full edge case
            self.short_term_queue
                .push(None)
                .map_err(|_| Error::CannotReceive)?;
            self.short_term_queue.swap_remove(remove_idx);
            // TODO fix rotation when long_term_queue is full - use HB and try recent() before full
            // iteration?
            self.long_term_queue
                .insert(message_hash)
                .map_err(|_| Error::CannotReceive)?;
            return Ok(StoreRecvOutcome::Seen);
        }

        if self.long_term_queue.contains(&message_hash) {
            return Ok(StoreRecvOutcome::Seen);
        }

        // if not, store hash and body and enqueue to short term queue with a random timeout
        let when = self.get_interval();
        let item = ShortTermQueueItem {
            message_hash,
            message_data_part,
            when,
        };
        self.short_term_queue
            .push(Some(item))
            .map_err(|_| Error::CannotReceive)?;
        Ok(StoreRecvOutcome::NotSeenScheduled(when))
    }

    /// supposed to be driven by a timer, if current tick >= some of the scheduled ticks in the
    /// short term queue it means, the message was not seen during the timeout interval and should
    /// be scheduled for retransmission into the tx queue
    pub fn tick_try_send(&mut self) -> Result<Vec<Message, U3>, ()> {
        let mut result = Vec::<Message, U3>::new();
        let mut idx: usize = 0;
        let mut remove_indices = Vec::<usize, U3>::new();

        // FIXME only gather indices and swap_remove them one by on in other cycle
        for item in &self.short_term_queue {
            if let Some(item) = item {
                // TODO if the short_term_queue is sorted, then we could break early here
                if item.when == self.tick_count {
                    // TODO remove from queue
                    result
                        .push(
                            Message::try_from_hash_data(
                                item.message_hash.clone(),
                                item.message_data_part.clone(),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                    remove_indices.push(idx).unwrap();
                }
            }
            idx += 1;
        }

        for idx in &remove_indices {
            self.short_term_queue.push(None).unwrap();
            self.short_term_queue.swap_remove(*idx).unwrap();
        }

        if self.tick_count == ShortTermQueueLength::U16 - 1 {
            self.tick_count = 0;
        } else {
            self.tick_count += 1;
        }

        Ok(result)
    }

    // will produce 0-ShortTermQueueLength, this will need tuning when timer set up
    fn get_interval(&mut self) -> u16 {
        let mut interval =
            self.tick_count + (self.rng.next_u32() % ShortTermQueueLength::U32) as u16;
        // handle rollover
        if interval > ShortTermQueueLength::U16 {
            interval = interval - ShortTermQueueLength::U16 - 1;
        }
        interval
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

    mod store {
        use super::*;

        use rand::Error;

        #[derive(Debug)]
        struct TestingRng(u8, Vec<u64, U64>);

        impl RngCore for TestingRng {
            fn next_u32(&mut self) -> u32 {
                self.next_u64() as u32
            }

            fn next_u64(&mut self) -> u64 {
                let next = &self.1[self.0 as usize];
                if self.0 == 63 {
                    self.0 = 0;
                } else {
                    self.0 += 1;
                }
                *next
            }

            fn fill_bytes(&mut self, dest: &mut [u8]) {
                let mut left = dest;
                while left.len() >= 8 {
                    let (l, r) = { left }.split_at_mut(8);
                    left = r;
                    let chunk: [u8; 8] = self.next_u64().to_le_bytes();
                    l.copy_from_slice(&chunk);
                }
                let n = left.len();
                if n > 4 {
                    let chunk: [u8; 8] = self.next_u64().to_le_bytes();
                    left.copy_from_slice(&chunk[..n]);
                } else if n > 0 {
                    let chunk: [u8; 4] = self.next_u32().to_le_bytes();
                    left.copy_from_slice(&chunk[..n]);
                }
            }

            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
                Ok(self.fill_bytes(dest))
            }
        }

        #[test]
        fn test_store_receive_not_seen() {
            let rng = SmallRng::seed_from_u64(0x1111_2222_3333_4444);
            let mut store = MessageStore::new(rng);
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );

            let outcome = store.recv(m).unwrap();
            // 555 is first value generated from the provided seed
            let expected_when = 555 % (ShortTermQueueLength::U16 - 1);
            assert_eq!(StoreRecvOutcome::NotSeenScheduled(expected_when), outcome);
        }

        #[test]
        fn test_store_receive_seen() {
            let rng = SmallRng::seed_from_u64(0x1111_2222_3333_4444);
            let mut store = MessageStore::new(rng);
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );
            let m_clone = m.clone();
            let m_clone_2 = m.clone();

            store.recv(m).unwrap();
            let outcome = store.recv(m_clone).unwrap();
            assert_eq!(StoreRecvOutcome::Seen, outcome);
            let outcome = store.recv(m_clone_2).unwrap();
            assert_eq!(StoreRecvOutcome::Seen, outcome);
        }

        #[test]
        fn test_schedue_tick_simple() {
            let rng = TestingRng(0, Vec::from_slice(&[2, 1]).unwrap());
            let mut store = MessageStore::new(rng);
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );

            let outcome = store.recv(m.clone()).unwrap();
            assert_eq!(StoreRecvOutcome::NotSeenScheduled(2), outcome);
            let tick_1_result = store.tick_try_send().unwrap();
            assert!(tick_1_result.is_empty());
            // ignore this one, should return at tick 3
            store.tick_try_send().unwrap();

            // now at tick 3 the vector of messages to resend should contain exactle 1 message - m
            let tick_3_result = store.tick_try_send().unwrap();
            assert_eq!(tick_3_result.len(), 1);
            assert_eq!(tick_3_result[0], m);
        }

        #[test]
        fn test_schedule_tick_rollover() {
            // max tick count is say 10 [0-9]
            // tick count is 8 (we are at tick 7)
            // when generated for message is 4 ticks, so it should be scheduled for 1 in the next
            // epoch
            let rng = TestingRng(0, Vec::from_slice(&[6, 1, 1, 1]).unwrap());
            let mut store = MessageStore::new(rng);
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );

            // lets move 3 ticks before rollover
            for _ in 0..(ShortTermQueueLength::USIZE - 3) {
                store.tick_try_send().unwrap();
                // println!("storge tick = {:?}", store);
            }

            let outcome = store.recv(m.clone()).unwrap();
            assert_eq!(StoreRecvOutcome::NotSeenScheduled(2), outcome);

            // lets move 5 ticks further
            for _ in 0..5 {
                let tick_result = store.tick_try_send().unwrap();
                assert_eq!(tick_result.len(), 0);
            }
            let tick_result = store.tick_try_send().unwrap();
            assert_eq!(tick_result.len(), 1);
            assert_eq!(tick_result[0], m);
        }
    }
}
