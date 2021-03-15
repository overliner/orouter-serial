//! Defines and implements protocol used on physical radio layer (now using LoRa)
//!
//! Logical [crate::overline::OverlineMessage] length can be [crate::overline::MaxLoraPayloadLength](255 B), but LoRa can only transmit 255B in
//! one message. MessageSlicer takes care of splitting message to appropriate number of parts with
//! necessary header information. On the receiving end these has to be assembled back to a logical
//! message - this is job of MessagePool
//
// TODO Remaining tasks from .plan
// * flush out unfinished messages from MessagePool after some time?
// * - else what happens when a lost of unreceived parts blocks out the Pool for newer ones
// * 7th message with 4th unmatching prefix comes in
// * slicer CRC16 creation
// * CRC16 at the end of P2pmessage - what does it check, whole message? only the data part?
// * parts_vec could take Option<P2pMessagePart> for nicer api and lower alloc size with resize_default
// * if message starts with 0xAA 0xAA -> single and has no header - can send up to 253B this way

use heapless::{FnvIndexMap, Vec};
use rand::prelude::*;
use typenum::{consts::*, op, Unsigned};

/// P2pMessagePart represents raw chunk of data received using radio chip.
/// It uses following structure:
///
/// | name        | length in bytes | description                                            |
/// |--:-:--------|--:-:------------|--:-:---------------------------------------------------|
/// | prefix      | 4               | prefix grouping parts of the message to one            |
/// | part_num    | 1               | part number 1, 2 or 3 (only 3-part messages supported) |
/// | total_count | 1               | total count of messages with this prefix               |
/// | length      | 1               | length of data                                         |
/// | data        | 1 - 248         | actual data                                            |
pub type MaxLoraMessageSize = U255;
pub type P2pMessagePart = Vec<u8, MaxLoraMessageSize>;

type MaxOverlineMessageLength = U512;
type MaxP2pMessagePartCount = U3;
type MaxUnfinishedP2pMessageCount = U4;

/// Represents a raw p2p message constructed back from chunks
/// This has yet to be parsed into a typed [overline
/// message](../overline/enum.OverlineMessageType.html)
pub type P2pMessage = Vec<u8, MaxOverlineMessageLength>;

type PrefixLength = U4;
type PrefixIdx = U0;
type Prefix = Vec<u8, PrefixLength>;

type PartNumberLength = U1;
type PartNumberIdx = op!(PrefixLength + PartNumberLength - U1);

type TotalCountLength = U1;
type TotalCountIdx = op!(PrefixLength + PartNumberLength + TotalCountLength - U1);

type LengthLength = U1;
type LengthIdx = op!(PrefixLength + PartNumberLength + TotalCountLength + LengthLength - U1);

type HeaderLength = op!(PrefixLength + PartNumberLength + TotalCountLength + LengthLength);

#[derive(Debug, PartialEq)]
pub enum Error {
    PoolFull,
    MalformedMessage,
    TooLong,
}

/// Holds parts of multipart messages before all parts have arrived
/// Maximum of 3 different sets of incomplete messages can be stored
///
/// Messages in vector under each prefix key are inserted at the correct index - are kept sorted
/// example of the map
/// {
///     '4B prefix': \[\[part_1\], \[part_2\], \[part3\]\],
///     ...
///     ...
/// }
#[derive(Default)]
pub struct MessagePool {
    // TODO holds reference to TX channel of the queue
    /// Contains parts of the raw P2pMessage. Parts are stored without the prefix
    incomplete_message_map: FnvIndexMap<
        Prefix,
        Vec<P2pMessagePart, MaxP2pMessagePartCount>,
        MaxUnfinishedP2pMessageCount,
    >,
}

impl MessagePool {
    // FIXME parts_vec could be Option<P2pMessagePart> - save empty vec creation and nicer api
    pub fn try_insert(&mut self, msg: P2pMessagePart) -> Result<Option<P2pMessage>, Error> {
        if !self.is_valid_message(&msg) {
            return Err(Error::MalformedMessage);
        }

        let part_num = &msg[PartNumberIdx::USIZE];
        let total_count = &msg[TotalCountIdx::USIZE];

        if *part_num == PartNumberLength::U8 && *total_count == TotalCountLength::U8 {
            // TODO check CRC
            return Ok(Some(
                Vec::<_, _>::from_slice(&msg[HeaderLength::USIZE..]).unwrap(),
            ));
        }

        let prefix = Vec::<_, _>::from_slice(&msg[PrefixIdx::USIZE..PrefixLength::USIZE]).unwrap();

        // get the parts vec
        let parts_vec = match self.incomplete_message_map.get_mut(&prefix) {
            Some(parts) => parts,
            None => {
                if self.incomplete_message_map.len() == MaxUnfinishedP2pMessageCount::U8 as usize {
                    return Err(Error::PoolFull);
                }
                let v = Vec::<P2pMessagePart, MaxP2pMessagePartCount>::new();
                // println!("inserting prefix = {:02x?}", prefix);
                self.incomplete_message_map
                    .insert(prefix.clone(), v)
                    .unwrap();
                self.incomplete_message_map.get_mut(&prefix).unwrap()
            }
        };
        let parts_index = part_num - 1;

        match parts_vec.get(parts_index as usize) {
            Some(part) if !part.is_empty() => {} // we already have this message part, not a problem
            Some(_) => {
                parts_vec[parts_index as usize] =
                    Vec::<_, _>::from_slice(&msg[HeaderLength::USIZE..]).unwrap();
            }
            None => {
                // lets insert the message
                parts_vec.resize_default(parts_index as usize + 1).unwrap();
                parts_vec[parts_index as usize] =
                    Vec::<_, _>::from_slice(&msg[HeaderLength::USIZE..]).unwrap();
            }
        }

        let has_all = parts_vec.iter().all(|current| !current.is_empty());
        // println!("has_all = {}", has_all);
        let has_all = has_all && parts_vec.len() == *total_count as usize;
        // println!("has_all = {}", has_all);
        // println!("parts_vec = {:?}", parts_vec);
        // for (i, val) in parts_vec.iter().enumerate() {
        //     println!("i = {}, val = {:?}, is_empty = {}", i, val, val.is_empty());
        // }

        if has_all {
            // FIXME check the CRC
            let mut r = Vec::<u8, MaxOverlineMessageLength>::new();
            for part in parts_vec.iter() {
                r.extend_from_slice(&part).unwrap();
            }

            self.incomplete_message_map.remove(&prefix).unwrap();

            return Ok(Some(r));
        }

        Ok(None)
    }

    pub fn size(&self) -> u8 {
        let mut size: u8 = 0;

        for incomplete_message_parts in self.incomplete_message_map.values() {
            for part in incomplete_message_parts {
                if !part.is_empty() {
                    size += 1
                }
            }
        }

        size
    }

    pub fn reset(&mut self) {
        self.incomplete_message_map.clear();
    }

    pub(crate) fn is_valid_message(&self, msg: &[u8]) -> bool {
        // 4B hash + part_num + total_count + len + 1B data
        if msg.len() < 8 {
            return false;
        }
        let part_num = &msg[PartNumberIdx::USIZE];
        let total_count = &msg[TotalCountIdx::USIZE];
        let len = &msg[LengthIdx::USIZE];
        // println!(
        //     "part_num_i = {}, total_count_i = {}, len_i = {}, MESSAGE_DATA_START_IDX = {}",
        //     MESSAGE_PART_NUMBER_IDX,
        //     MESSAGE_PART_TOTAL_COUNT_IDX,
        //     MESSAGE_PART_LEN_IDX,
        //     MESSAGE_DATA_START_IDX
        // );

        // part num can be 1, 2 or 3
        if !(1..4).contains(part_num) {
            return false;
        }

        // total_count has to be in [1;3] interval
        if !(1..4).contains(total_count) {
            return false;
        }

        // max data lenght can be 248 (255-header length)
        if !(1..249).contains(len) {
            return false;
        }

        // claimed len is different from actual remaining data bytes
        if *len as usize != msg.len() - HeaderLength::USIZE {
            return false;
        }

        true
    }
}

/// Takes care of splitting OVerline logical chunks of data [TODO link to Overline message]() to
/// chunks transmittable using LoRa with a header allowing receiver to assemble the logical message
/// back from received parts
pub struct MessageSlicer {
    rng: SmallRng,
}

impl MessageSlicer {
    pub fn new(initial_seed: u64) -> Self {
        MessageSlicer {
            rng: SmallRng::seed_from_u64(initial_seed),
        }
    }

    pub fn slice(
        &mut self,
        msg: &[u8],
    ) -> Result<Vec<P2pMessagePart, MaxP2pMessagePartCount>, Error> {
        if msg.len() > MaxOverlineMessageLength::USIZE {
            return Err(Error::TooLong);
        }

        let mut prefix = Prefix::new();
        // FIXME - this must be possible to do nicer
        for _ in 0..4 {
            prefix.push(self.rng.gen()).unwrap();
        }
        let mut res = Vec::<P2pMessagePart, MaxP2pMessagePartCount>::new();
        let chunks = msg.chunks(MaxLoraMessageSize::USIZE - HeaderLength::USIZE);
        let total_count = chunks.len();
        for (i, part_bytes) in chunks.enumerate() {
            let mut p = P2pMessagePart::new();
            p.extend_from_slice(&prefix).unwrap();
            p.push(i as u8 + 1).unwrap(); // part_num
            p.push(total_count as u8).unwrap(); // total_count
            p.push(part_bytes.len() as u8).unwrap(); // length
            p.extend_from_slice(&part_bytes).unwrap(); // data
            res.push(p).unwrap();
            // FIXME append the CRC
        }

        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pool_try_insert_1_of_1() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x01, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let res = p.try_insert(msg1).unwrap().unwrap();
        assert_eq!(&res, &[0xc0, 0xff, 0xee]);
    }

    #[test]
    fn test_pool_try_insert_1_of_2() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x02, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let res = p.try_insert(msg1).unwrap();
        assert_eq!(res, None);
    }

    #[test]
    fn test_pool_try_insert_1_and_2_of_2() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x02, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();

        p.try_insert(msg1).unwrap();
        let res = p.try_insert(msg2).unwrap().unwrap();
        assert_eq!(&res, &[0xc0, 0xff, 0xee, 0xc1, 0xff, 0xee]);
    }

    #[test]
    fn test_pool_try_insert_2_and_1_of_2() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x02, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();

        assert_eq!(None, p.try_insert(msg2).unwrap());
        let res = p.try_insert(msg1).unwrap().unwrap();
        assert_eq!(&res, &[0xc0, 0xff, 0xee, 0xc1, 0xff, 0xee]);
    }

    #[test]
    fn test_pool_try_insert_1_2_and_3_of_3() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x03, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x02, 0x03, 0x03, 0xc1, 0xff, 0xee]).unwrap();

        let msg3 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x03, 0x03, 0x03, 0xc2, 0xff, 0xee]).unwrap();

        assert_eq!(None, p.try_insert(msg1).unwrap());
        assert_eq!(None, p.try_insert(msg2).unwrap());
        let res = p.try_insert(msg3).unwrap().unwrap();
        assert_eq!(
            &res,
            &[0xc0, 0xff, 0xee, 0xc1, 0xff, 0xee, 0xc2, 0xff, 0xee]
        );
    }

    #[test]
    fn test_pool_try_insert_2_1_and_3_of_3() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x03, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x02, 0x03, 0x03, 0xc1, 0xff, 0xee]).unwrap();

        let msg3 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x03, 0x03, 0x03, 0xc2, 0xff, 0xee]).unwrap();

        assert_eq!(None, p.try_insert(msg2).unwrap());
        assert_eq!(None, p.try_insert(msg1).unwrap());
        let res = p.try_insert(msg3).unwrap().unwrap();
        assert_eq!(
            &res,
            &[0xc0, 0xff, 0xee, 0xc1, 0xff, 0xee, 0xc2, 0xff, 0xee]
        );
    }

    #[test]
    fn test_pool_try_insert_3_1_and_2_of_3() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x03, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x02, 0x03, 0x03, 0xc1, 0xff, 0xee]).unwrap();

        let msg3 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x03, 0x03, 0x03, 0xc2, 0xff, 0xee]).unwrap();

        assert_eq!(None, p.try_insert(msg3).unwrap());
        assert_eq!(None, p.try_insert(msg1).unwrap());
        let res = p.try_insert(msg2).unwrap().unwrap();
        assert_eq!(
            &res,
            &[0xc0, 0xff, 0xee, 0xc1, 0xff, 0xee, 0xc2, 0xff, 0xee]
        );
    }

    #[test]
    fn test_pool_try_insert_3_1_and_3_again_of3() {
        let mut p = MessagePool::default();
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x03, 0x03, 0xc0, 0xff, 0xee]).unwrap();

        let msg3 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x03, 0x03, 0x03, 0xc2, 0xff, 0xee]).unwrap();

        assert_eq!(None, p.try_insert(msg3.clone()).unwrap());
        assert_eq!(None, p.try_insert(msg1).unwrap());
        let res = p.try_insert(msg3).unwrap();
        assert_eq!(res, None);
    }

    #[test]
    fn test_try_insert_pool_full() {
        let mut p = MessagePool::default();
        // messages with 5 different prefixes
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x02, 0x03, 0xc0, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg1).unwrap());
        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x02, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg2).unwrap());
        let msg3 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x03, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg3).unwrap());
        let msg4 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x04, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg4).unwrap());
        let msg5 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x05, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(Err(Error::PoolFull), p.try_insert(msg5));
    }

    #[test]
    fn test_pool_size() {
        let mut p = MessagePool::default();
        assert_eq!(p.size(), 0);
        // messages with 5 different prefixes
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x02, 0x03, 0xc0, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg1).unwrap());
        assert_eq!(p.size(), 1);
        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x02, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg2).unwrap());
        assert_eq!(p.size(), 2);
        let msg3 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x03, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg3).unwrap());
        assert_eq!(p.size(), 3);
        let msg4 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x04, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg4).unwrap());
        assert_eq!(p.size(), 4);

        let msg1_2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc0, 0xff, 0xee]).unwrap();
        assert_ne!(None, p.try_insert(msg1_2).unwrap());
        assert_eq!(p.size(), 3); // none added, msg1 removed with this one
    }

    #[test]
    fn test_pool_reset() {
        let mut p = MessagePool::default();
        assert_eq!(p.size(), 0);
        // messages with 5 different prefixes
        let msg1 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x01, 0x01, 0x02, 0x02, 0x01, 0x02, 0x03, 0xc0, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg1).unwrap());
        assert_eq!(p.size(), 1);
        let msg2 =
            //                         prefix               | num | tot | len | data
            Vec::<_, _>::from_slice(&[0x02, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]).unwrap();
        assert_eq!(None, p.try_insert(msg2.clone()).unwrap());
        assert_eq!(p.size(), 2);

        p.reset();
        assert_eq!(p.size(), 0);

        assert_eq!(None, p.try_insert(msg2).unwrap());
        assert_eq!(p.size(), 1);

        p.reset();
        assert_eq!(p.size(), 0);
    }

    #[test]
    fn test_is_valid_message_valid() {
        let p = MessagePool::default();
        //                             prefix               | num | tot | len | data
        assert!(p.is_valid_message(&[0x01, 0x01, 0x02, 0x02, 0x02, 0x02, 0x03, 0xc1, 0xff, 0xee]));
    }

    #[test]
    fn test_is_valid_message_shorter_than_possible() {
        let p = MessagePool::default();
        //                             prefix               | num | tot | len | data
        assert!(!p.is_valid_message(&[0xff, 0xff, 0xff, 0xff, 0x04, 0x01, 0x02]));
    }

    #[test]
    fn test_is_valid_message_wrong_num() {
        let p = MessagePool::default();
        //                             prefix               | num | tot | len | data
        assert!(!p.is_valid_message(&[0xff, 0xff, 0xff, 0xff, 0x04, 0x01, 0x02, 0xff, 0xff]));
    }

    #[test]
    fn test_is_valid_message_wrong_total_num() {
        let p = MessagePool::default();
        //                             prefix               | num | tot | len | data
        assert!(!p.is_valid_message(&[0xff, 0xff, 0xff, 0xff, 0x01, 0x04, 0x02, 0xff, 0xff]));
    }

    #[test]
    fn test_is_valid_message_wrong_len() {
        let p = MessagePool::default();
        //                             prefix               | num | tot | len | data
        assert!(!p.is_valid_message(&[0xff, 0xff, 0xff, 0xff, 0x01, 0x03, 0xf9, 0xff, 0xff]));
    }

    #[test]
    fn test_is_valid_message_not_matching_claimed_and_actual_len() {
        let p = MessagePool::default();
        //                             prefix               | num | tot | len | data
        assert!(!p.is_valid_message(&[0xff, 0xff, 0xff, 0xff, 0x01, 0x03, 0x03, 0xff, 0xff]));
    }

    #[test]
    fn test_slicer_single_message() {
        let mut s = MessageSlicer::new(0xdead_beef_cafe_d00d);
        let parts = s.slice(&[0xc0, 0xff, 0xee]).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(
            parts[0],
            //   prefix              |part_n|tot|part_l|data
            &[0x3c, 0x2a, 0x2b, 0xed, 0x01, 0x01, 0x03, 0xc0, 0xff, 0xee]
        );
    }

    #[test]
    fn test_slicer_two_parts() {
        let mut s = MessageSlicer::new(0xdead_beef_cafe_d00d);
        let mut test_data_message = Vec::<u8, MaxOverlineMessageLength>::new();
        for b in core::iter::repeat(0xff).take(MaxLoraMessageSize::USIZE - HeaderLength::USIZE) {
            test_data_message.push(b).unwrap();
        }
        test_data_message
            .extend_from_slice(&[0xc0, 0xff, 0xee])
            .unwrap();
        let parts = s.slice(&test_data_message).unwrap();
        assert_eq!(parts.len(), 2);
        // TODO test part 1
        assert_eq!(
            parts[1],
            //   prefix              |part_n|tot|part_l|data
            &[0x3c, 0x2a, 0x2b, 0xed, 0x02, 0x02, 0x03, 0xc0, 0xff, 0xee]
        );
    }

    #[test]
    fn test_slicer_too_long_data() {
        let mut s = MessageSlicer::new(0xdead_beef_cafe_d00d);
        let mut test_data_message = Vec::<u8, U513>::new();
        for b in core::iter::repeat(0xff).take(MaxOverlineMessageLength::USIZE + 1) {
            test_data_message.push(b).unwrap();
        }
        assert_eq!(Err(Error::TooLong), s.slice(&test_data_message));
    }
}
