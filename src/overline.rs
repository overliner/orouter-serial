use heapless::{consts::*, Vec};

pub type MaxLoraPayloadLength = U255;
pub type OverlineMessageHash = Vec<u8, U16>;

/// Logical message of overline protocol - does not contain any link level data
/// (e.g. magic byte, message type, or information about how 512B message was transferred)
pub enum OverlineMessageType {
    Challenge(Vec<u8, U16>),
    Proof(Vec<u8, U16>),
    Flush(Vec<u8, U16>),
    Receipt(Vec<u8, U16>),
    Other(Vec<u8, U512>),
}

#[cfg(test)]
mod tests {
    // use super::*;
    #[test]
    fn test_message_serialize() {
        assert!(true);
    }
}
