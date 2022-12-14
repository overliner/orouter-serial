use defmt::Formatter;

use crate::host::{
    codec, Error as HostError, Message as HostMessage, ParseMessageError, StatusCode,
};
use crate::overline::{
    Error as OverlineError, Message as OverlineMessage, MessageType, ShortTermQueueItem,
    StoreRecvOutcome,
};

impl defmt::Format for HostError {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "HostError::");
        match self {
            HostError::BufferFull => defmt::write!(fmt, "BufferFull"),
            HostError::BufferLengthNotSufficient => defmt::write!(fmt, "BufferLengthNotSufficient"),
            HostError::MalformedMessage => defmt::write!(fmt, "MalformedMessage"),
            HostError::MessageQueueFull => defmt::write!(fmt, "MessageQueueFull"),
            HostError::CannotAppendCommand => defmt::write!(fmt, "CannotAppendCommand"),
            HostError::CodecError(e) => defmt::write!(fmt, "CodecError({:?})", e),
        }
    }
}

impl defmt::Format for codec::CodecError {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "CodecError::");
        match self {
            codec::CodecError::FrameCreateError => defmt::write!(fmt, "FrameCreateError"),
            codec::CodecError::DecodeError => defmt::write!(fmt, "DecodeError"),
            codec::CodecError::MalformedHex(_) => defmt::write!(fmt, "MalformedHex"),
        }
    }
}

impl defmt::Format for HostMessage {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "Message::");
        match self {
            HostMessage::SendData { data } => defmt::write!(fmt, "SendData {{ data: {=[u8]:x} }}", data),
            HostMessage::ReceiveData { data } => defmt::write!(fmt, "ReceiveData {{ data: {=[u8]:x} }}", data),
            HostMessage::Configure { region } => defmt::write!(fmt, "Configure {{ region: {=u8} }}", region),
            HostMessage::ReportRequest => defmt::write!(fmt, "ReportRequest"),
            HostMessage::Report {
                sn,
                version_data,
                region,
                receive_queue_size,
                transmit_queue_size,
            } => defmt::write!(fmt, "Report {{ sn: {=u32}, version_data: {=u32}, region: {=u8}, receive_queue_size: {=u8}, transmit_queue_size: {=u8} }}", sn, version_data, region, receive_queue_size, transmit_queue_size),
            HostMessage::Status { code } => defmt::write!(fmt, "Status({=?})", code),
            HostMessage::UpgradeFirmwareRequest => defmt::write!(fmt, "UpgradeFirmwareRequest"),
            HostMessage::SetTimestamp { timestamp } => defmt::write!(fmt, "SetTimestamp({=u64})", timestamp),
            HostMessage::GetRawIq => defmt::write!(fmt, "GetRawIq"),
            HostMessage::RawIq { data } => defmt::write!(fmt, "RawIq {{ data: {=[u8]:x} }}", data)
        }
    }
}

impl defmt::Format for ParseMessageError {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "ParseMessageError::");
        match self {
            ParseMessageError::MissingSeparator => defmt::write!(fmt, "MissingSeparator"),
            ParseMessageError::InvalidMessage => defmt::write!(fmt, "InvalidMessage"),
            ParseMessageError::InvalidHex(_) => defmt::write!(fmt, "InvalidHex"),
            ParseMessageError::InvalidPayloadLength => defmt::write!(fmt, "InvalidPayloadLength"),
            ParseMessageError::PayloadTooLong => defmt::write!(fmt, "PayloadTooLong"),
        }
    }
}

impl defmt::Format for StatusCode {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "StatusCode::");
        match self {
            StatusCode::FrameReceived => defmt::write!(fmt, "FrameReceived"),
            StatusCode::CommandReceived => defmt::write!(fmt, "CommandReceived"),
            StatusCode::ErrUnknownCommmandReceived => {
                defmt::write!(fmt, "ErrUnknownCommmandReceived")
            }
            StatusCode::ErrBusyLoraTransmitting => defmt::write!(fmt, "ErrBusyLoraTransmitting"),
            StatusCode::ErrMessageQueueFull => defmt::write!(fmt, "ErrMessageQueueFull"),
            StatusCode::RadioNotConfigured => defmt::write!(fmt, "RadioNotConfigured"),
        }
    }
}

impl defmt::Format for OverlineError {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "OverlineError::");
        match self {
            OverlineError::InvalidMessage => defmt::write!(fmt, "InvalidMessage"),
            OverlineError::ShortTermQueueFull => defmt::write!(fmt, "ShortTermQueueFull"),
            OverlineError::LongTermQueueFull => defmt::write!(fmt, "LongTermQueueFull"),
            OverlineError::CannotReceive => defmt::write!(fmt, "CannotReceive"),
            OverlineError::UnknownType => defmt::write!(fmt, "UnknownType"),
        }
    }
}

impl defmt::Format for OverlineMessage {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "Message({=[u8]:x})", self.0);
    }
}

impl defmt::Format for MessageType {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "MessageType::");
        match self {
            MessageType::Data => defmt::write!(fmt, "Data"),
            MessageType::Challenge => defmt::write!(fmt, "Challenge"),
            MessageType::Proof => defmt::write!(fmt, "Proof"),
            MessageType::Flush => defmt::write!(fmt, "Flush"),
            MessageType::Receipt => defmt::write!(fmt, "Receipt"),
            MessageType::Other => defmt::write!(fmt, "Other"),
        }
    }
}

impl defmt::Format for ShortTermQueueItem {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "ShortTermQueueItem {{\n");
        defmt::write!(fmt, "\twhen: {=u16}\n", self.when);
        defmt::write!(fmt, "\tmessage_hash: {=[u8]:x}\n", self.message_hash);
        defmt::write!(
            fmt,
            "\tmessage_data_part: {=[u8]:x}\n",
            self.message_data_part
        );
        defmt::write!(fmt, "}}");
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
