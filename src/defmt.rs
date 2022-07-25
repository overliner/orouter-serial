use defmt::Formatter;

use crate::host::{Error as HostError, Message as HostMessage, ParseMessageError, StatusCode};
use crate::overline::{
    Error as OverlineError, Message as OverlineMessage, MessageType, ShortTermQueueItem,
    StoreRecvOutcome,
};

impl defmt::Format for HostError {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for HostMessage {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for ParseMessageError {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for StatusCode {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for OverlineError {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for OverlineMessage {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for MessageType {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for ShortTermQueueItem {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "{=?}", self)
    }
}

impl defmt::Format for StoreRecvOutcome {
    fn format(&self, fmt: Formatter<'_>) {
        match self {
            StoreRecvOutcome::NotSeenScheduled(s) => {
                defmt::write!(fmt, "Outcome::NotSeenScheduled, {=u16}", s)
            }
            StoreRecvOutcome::Seen => defmt::write!(fmt, "Outcome::Seen"),
            StoreRecvOutcome::Command => defmt::write!(fmt, "Outcome::Command"),
        }
    }
}
