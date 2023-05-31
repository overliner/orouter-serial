//! Defines Overline protocol
//!
//! Describes types and structure of logical overline message - how it is represented in the
//! physical LoRa message. Defines utility struct [MessageStore] which enabled
//! implementation of overline message retransmission rules
use core::cmp::{Ord, Ordering};

use heapless::{HistoryBuffer, Vec};
use rand::prelude::*;

pub const MAX_LORA_PAYLOAD_LENGTH: usize = 255;
pub const OVERLINE_STORE_MESSAGE_HASH_LENGTH: usize = 16;
pub const MESSAGE_MAX_DATA_LENGTH: usize =
    MAX_LORA_PAYLOAD_LENGTH - OVERLINE_STORE_MESSAGE_HASH_LENGTH;
pub type MessageDataPart = Vec<u8, MESSAGE_MAX_DATA_LENGTH>; // FIXME better naming
pub type MessageHash = Vec<u8, OVERLINE_STORE_MESSAGE_HASH_LENGTH>;

#[derive(PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum Error {
    InvalidMessage,
    ShortTermQueueFull,
    LongTermQueueFull,
    CannotReceive,
    UnknownType,
}

/// Logical message of overline protocol - does not contain any link level data
/// (e.g. magic byte, message type, or information about how 512B message was transferred)
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct Message(pub(crate) Vec<u8, MAX_LORA_PAYLOAD_LENGTH>);

// FIXME implement into host::Message::SendData and host::Message::ReceiveData
/// This represents an overline message for purpose of working with is in the firmware. This is not
/// an equivalent of wireless_protocol::Message. Data passed to new method should already have been
/// validated using wireless_protocol::is_valid_message
impl Message {
    pub fn new(data: Vec<u8, MAX_LORA_PAYLOAD_LENGTH>) -> Self {
        Message(data)
    }

    pub fn try_from_hash_data(hash: MessageHash, data: MessageDataPart) -> Result<Self, Error> {
        if hash.len() != OVERLINE_STORE_MESSAGE_HASH_LENGTH {
            return Err(Error::InvalidMessage);
        }

        if hash.len() + data.len() > MAX_LORA_PAYLOAD_LENGTH {
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
        if self.0.len() < OVERLINE_STORE_MESSAGE_HASH_LENGTH {
            return Err(Error::InvalidMessage);
        }

        match MessageHash::from_slice(&self.0[0..OVERLINE_STORE_MESSAGE_HASH_LENGTH]) {
            Ok(h) => Ok(h),
            Err(()) => Err(Error::InvalidMessage),
        }
    }

    pub fn data_part(&self) -> Result<MessageDataPart, Error> {
        match MessageDataPart::from_slice(&self.0[OVERLINE_STORE_MESSAGE_HASH_LENGTH..]) {
            Ok(h) => Ok(h),
            Err(()) => Err(Error::InvalidMessage),
        }
    }

    pub fn typ(&self) -> Result<wireless_protocol::MessageType, Error> {
        match self.0[wireless_protocol::MSG_TYPE_IDX] {
            0x01 => Ok(wireless_protocol::MessageType::Data),
            0x02 => Ok(wireless_protocol::MessageType::Challenge),
            0x03 => Ok(wireless_protocol::MessageType::Proof),
            0x04 => Ok(wireless_protocol::MessageType::Flush),
            0x05 => Ok(wireless_protocol::MessageType::Receipt),
            0xff => Ok(wireless_protocol::MessageType::Other),
            _ => Err(Error::UnknownType),
        }
    }

    pub fn to_vec(self) -> Vec<u8, MAX_LORA_PAYLOAD_LENGTH> {
        self.0
    }
}

/// Describes outcome of attempt to [`MessageStore::recv`]
#[derive(PartialEq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub enum StoreRecvOutcome {
    /// hash was not in the short term queue, scheduled for retransmission
    NotSeenScheduled(u16),
    /// message was seen, removed from short term queue
    Seen,
    /// message was a command
    Command,
}

/// Wraps message parts (hash and data) for storing them ordered in the short term queue
#[derive(Eq)]
#[cfg_attr(feature = "std", derive(Debug))]
pub struct ShortTermQueueItem {
    /// determines in how many ticks the item expires for the short term queue
    pub(crate) when: u16,
    pub(crate) message_hash: MessageHash,
    pub(crate) message_data_part: MessageDataPart,
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

pub const SHORT_TERM_QUEUE_LENGTH: usize = 64;
pub const LONG_TERM_QUEUE_LENGTH: usize = 512;

/// Store is responsible for applying rules for storing and possible retransmission of overline
/// messages seen by the node
pub struct MessageStore<'a, R: RngCore> {
    tick_count: u16,
    short_term_queue: &'a mut Vec<ShortTermQueueItem, SHORT_TERM_QUEUE_LENGTH>,
    long_term_queue: &'a mut HistoryBuffer<MessageHash, LONG_TERM_QUEUE_LENGTH>,
    rng: R,
}

impl<'a, R: RngCore> MessageStore<'a, R> {
    pub fn new(
        rng: R,
        short_term_queue: &'a mut Vec<ShortTermQueueItem, SHORT_TERM_QUEUE_LENGTH>,
        long_term_queue: &'a mut HistoryBuffer<MessageHash, LONG_TERM_QUEUE_LENGTH>,
    ) -> Self {
        MessageStore {
            tick_count: Default::default(),
            short_term_queue,
            long_term_queue,
            rng,
        }
    }

    /// used when node received a message
    pub fn recv(&mut self, message: Message) -> Result<StoreRecvOutcome, Error> {
        let (message_hash, message_data_part) = message.into_hash_data()?;
        // if we have seen this, and is in short term queue, immediately remove it from short term queue and store in long term queue
        let mut idx: usize = 0;
        let mut remove_idx = None;
        for item in self.short_term_queue.iter() {
            if item.message_hash == message_hash {
                remove_idx = Some(idx);
                break;
            }
            idx += 1;
        }

        if let Some(remove_idx) = remove_idx {
            self.short_term_queue.swap_remove(remove_idx);
            self.long_term_queue.write(message_hash);
            return Ok(StoreRecvOutcome::Seen);
        }

        for item in self.long_term_queue.as_slice() {
            if item == &message_hash {
                return Ok(StoreRecvOutcome::Seen);
            }
        }

        // if not, store hash and body and enqueue to short term queue with a random timeout
        let when = self.get_interval();
        let item = ShortTermQueueItem {
            message_hash,
            message_data_part,
            when,
        };

        if self.short_term_queue.len() == SHORT_TERM_QUEUE_LENGTH {
            return Err(Error::ShortTermQueueFull);
        }

        self.short_term_queue
            .push(item)
            .map_err(|_| Error::CannotReceive)?;
        Ok(StoreRecvOutcome::NotSeenScheduled(when))
    }

    /// supposed to be driven by a timer, if current tick >= some of the scheduled ticks in the
    /// short term queue it means, the message was not seen during the timeout interval and should
    /// be scheduled for retransmission into the tx queue
    pub fn tick_try_send(&mut self) -> Result<Vec<Message, 3>, ()> {
        let mut result = Vec::<Message, 3>::new();
        let mut idx: usize = 0;
        let mut remove_indices = Vec::<usize, 3>::new();

        // FIXME only gather indices and swap_remove them one by on in other cycle
        for item in self.short_term_queue.iter() {
            // TODO if the short_term_queue is sorted, then we could break early here
            if item.when == self.tick_count {
                // TODO remove from queue
                let msg = match Message::try_from_hash_data(
                    item.message_hash.clone(),
                    item.message_data_part.clone(),
                ) {
                    Ok(msg) => msg,
                    Err(_) => return Err(()),
                };
                match (result.push(msg), remove_indices.push(idx)) {
                    (Ok(_), Ok(_)) => {}
                    _ => return Err(()),
                }
            }
            idx += 1;
        }

        for idx in &remove_indices {
            self.short_term_queue.swap_remove(*idx);
        }

        for msg in &result {
            let hash = match msg.hash() {
                Ok(hash) => hash,
                Err(_) => return Err(()),
            };
            self.long_term_queue.write(hash);
        }

        if self.tick_count == SHORT_TERM_QUEUE_LENGTH as u16 - 1 {
            self.tick_count = 0;
        } else {
            self.tick_count += 1;
        }

        Ok(result)
    }

    pub fn mark_seen(&mut self, message: &Message) -> Result<(), Error> {
        if !self.long_term_queue.as_slice().contains(&message.hash()?) {
            self.long_term_queue.write(message.hash()?)
        }
        Ok(())
    }

    // will produce 0-ShortTermQueueLength, this will need tuning when timer set up
    fn get_interval(&mut self) -> u16 {
        let rng = self.rng.next_u32();
        let mut interval = self.tick_count + (rng % SHORT_TERM_QUEUE_LENGTH as u32) as u16;
        // handle rollover
        if interval > SHORT_TERM_QUEUE_LENGTH as u16 {
            interval = interval - SHORT_TERM_QUEUE_LENGTH as u16 - 1;
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
                0xbb, 0xcc, 0xef, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(Err(Error::UnknownType), m.typ())
    }

    #[test]
    fn test_typ_ok() {
        let m = Message(
            Vec::from_slice(&[
                0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0x02, 0xaa, 0xaa,
                0xbb, 0xcc, 0x02, 0xff,
            ])
            .unwrap(),
        );
        assert_eq!(wireless_protocol::MessageType::Challenge, m.typ().unwrap())
    }

    #[test]
    fn test_try_from_into_hash_data() {
        let hash = Vec::<u8, 16>::from_slice(&[
            // hash                                                           type
            0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10, 0xaa, 0x10, 0xaa, 0xff, 0xaa, 0x10,
            0xaa, 0x10,
        ])
        .unwrap();
        let data = Vec::<u8, 239>::from_slice(&[
            // some data ->
            0xef, 0xda, 0x1a, 0xda, 0x1a,
        ])
        .unwrap();

        let m = Message::try_from_hash_data(hash.clone(), data.clone()).unwrap();
        assert_eq!(hash, m.hash().unwrap());
        assert_eq!(wireless_protocol::MessageType::Other, m.typ().unwrap());

        // m moves here
        let (hash_new, data_new) = m.into_hash_data().unwrap();
        assert_eq!(hash, hash_new);
        assert_eq!(data, data_new);
    }

    mod store {
        use super::*;

        use rand::Error as RandError;

        #[cfg_attr(feature = "std", derive(Debug))]
        struct TestingRng(u8, Vec<u64, 64>);

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

            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RandError> {
                Ok(self.fill_bytes(dest))
            }
        }

        #[test]
        fn test_store_receive_not_seen() {
            let rng = TestingRng(0, Vec::from_slice(&[2, 1]).unwrap());
            let mut short_q = Vec::new();
            let mut long_q = HistoryBuffer::new();
            let mut store = MessageStore::new(rng, &mut short_q, &mut long_q);
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );

            let outcome = store.recv(m).unwrap();
            let expected_when = 2;
            assert_eq!(StoreRecvOutcome::NotSeenScheduled(expected_when), outcome);
        }

        #[test]
        fn test_store_receive_seen() {
            let rng = SmallRng::seed_from_u64(0x1111_2222_3333_4444);
            let mut short_q = Vec::new();
            let mut long_q = HistoryBuffer::new();
            let mut store = MessageStore::new(rng, &mut short_q, &mut long_q);
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
        fn test_store_receive_seen_not_clone() {
            let rng = TestingRng(0, Vec::from_slice(&[1, 1]).unwrap());
            let mut short_q = Vec::new();
            let mut long_q = HistoryBuffer::new();
            let mut store = MessageStore::new(rng, &mut short_q, &mut long_q);
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );
            let m2 = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );

            let _ = store.recv(m).unwrap();
            store.tick_try_send().unwrap(); // tick = 0
            store.tick_try_send().unwrap(); // tick = 1
            let outcome = store.recv(m2.clone()).unwrap();
            assert_eq!(StoreRecvOutcome::Seen, outcome);
            store.tick_try_send().unwrap(); // tick = 2
            let outcome = store.recv(m2).unwrap();
            assert_eq!(StoreRecvOutcome::Seen, outcome);
        }

        #[test]
        fn test_schedue_tick_simple() {
            let rng = TestingRng(0, Vec::from_slice(&[2, 1]).unwrap());
            let mut short_q = Vec::new();
            let mut long_q = HistoryBuffer::new();
            let mut store = MessageStore::new(rng, &mut short_q, &mut long_q);
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
            let mut short_q = Vec::new();
            let mut long_q = HistoryBuffer::new();
            let mut store = MessageStore::new(rng, &mut short_q, &mut long_q);
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );

            // lets move 3 ticks before rollover
            for _ in 0..(SHORT_TERM_QUEUE_LENGTH - 3) {
                store.tick_try_send().unwrap();
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

        #[test]
        fn test_schedule_max_plus_1_message() {
            let rng = TestingRng(
                0,
                Vec::from_slice(&[
                    0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                ])
                .unwrap(),
            );
            let mut short_q = Vec::new();
            let mut long_q = HistoryBuffer::new();
            let mut store = MessageStore::new(rng, &mut short_q, &mut long_q);

            // let's receive 16 messages - full length of the queue
            for n in 0..(SHORT_TERM_QUEUE_LENGTH) {
                let first_byte = (n + 100) as u8;
                let m = Message(
                    Vec::from_slice(&[
                        0xaa, first_byte, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                        0xaa, 0xaa, 0xaa, 0xbb, 0xcc, 0x15, 0xff,
                    ])
                    .unwrap(),
                );
                let _ = store.recv(m).unwrap();
            }

            // try to receive one more - and check proper error instead of outcome
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0x00, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );
            let outcome = store.recv(m);

            assert_eq!(Err(Error::ShortTermQueueFull), outcome);

            // now tick to remove one item from the queue
            let tick_result = store.tick_try_send().unwrap();
            assert_eq!(tick_result.len(), 1);
            let msg = tick_result[0].clone();
            let (hash, _) = msg.into_hash_data().unwrap();
            assert!(hash.starts_with(&[0xaa, 100])); // verify it's the first tick scheduled message

            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0x00, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );

            // store now should be able to receive another message, with when = 3 (tick_count = 1 +
            // rng_vec[1])
            let outcome = store.recv(m).unwrap();
            assert_eq!(StoreRecvOutcome::NotSeenScheduled(3), outcome);
        }

        #[test]
        fn test_schedule_max_plus_1_long_term_queue() {
            let rng = TestingRng(
                0,
                Vec::from_slice(&[
                    0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
                ])
                .unwrap(),
            );
            let mut short_q = Vec::new();
            let mut long_q = HistoryBuffer::new();
            let mut store = MessageStore::new(rng, &mut short_q, &mut long_q);

            // let's receive 512 messages - full length of the long term queue
            for n in 0..(LONG_TERM_QUEUE_LENGTH) {
                let bytes = (n as u16).to_be_bytes();
                let m = Message(
                    Vec::from_slice(&[
                        0xaa, bytes[0], bytes[1], 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                        0xaa, 0xaa, 0xaa, 0xbb, 0xcc, 0x15, 0xff,
                    ])
                    .unwrap(),
                );
                let _ = store.recv(m.clone()).unwrap();
                let outcome = store.recv(m).unwrap();
                assert_eq!(outcome, StoreRecvOutcome::Seen);
            }

            // try to add another message, this should make the long term queue to pop the oldest
            // hash
            let m = Message(
                Vec::from_slice(&[
                    0xaa, 0xff, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa, 0xaa,
                    0xaa, 0xbb, 0xcc, 0x15, 0xff,
                ])
                .unwrap(),
            );
            // for the first time it was not seen
            let outcome = store.recv(m.clone()).unwrap();
            assert_eq!(outcome, StoreRecvOutcome::NotSeenScheduled(0));
            // for the second time it was moved to long term queue, which didn't fail and returns
            // ::Seen
            let outcome = store.recv(m).unwrap();
            assert_eq!(outcome, StoreRecvOutcome::Seen);
        }
    }
}
