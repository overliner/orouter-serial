use defmt::Formatter;

use crate::host::{
    codec, Error as HostError, Message as HostMessage, ParseMessageError, StatusCode,
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
            HostError::CobsEncodeError => defmt::write!(fmt, "CobsEncodeError"),
            HostError::CannotParseConfigNetwork => defmt::write!(fmt, "CannotParseConfigNetwork"),
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
            HostMessage::Configure { region, spreading_factor, network } => defmt::write!(fmt, "Configure {{ region: {=u8}, spreading_factor: {=u8}, network: {=[u8]:x} }}", region, spreading_factor, network.to_be_bytes()),
            HostMessage::ReportRequest => defmt::write!(fmt, "ReportRequest"),
            HostMessage::Report {
                sn,
                version_data,
                region,
                spreading_factor,
                receive_queue_size,
                transmit_queue_size,
                network,
            } => defmt::write!(fmt, "Report {{ sn: {=u32}, version_data: {=[u8]:x}, region: {=u8}, spreading_factor: {=u8}, receive_queue_size: {=u8}, transmit_queue_size: {=u8}, network: {=[u8]:x} }}", sn, version_data, region, spreading_factor, receive_queue_size, transmit_queue_size, network.to_be_bytes()),
            HostMessage::Status { code } => defmt::write!(fmt, "Status({=?})", code),
            HostMessage::UpgradeFirmwareRequest => defmt::write!(fmt, "UpgradeFirmwareRequest"),
            HostMessage::SetTimestamp { timestamp } => defmt::write!(fmt, "SetTimestamp({=u64})", timestamp),
            HostMessage::GetRawIq => defmt::write!(fmt, "GetRawIq"),
            #[cfg(feature = "std")]
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
            ParseMessageError::InvalidHexConfigNetwork => {
                defmt::write!(fmt, "InvalidHexConfigNetwork")
            }
            ParseMessageError::MissingConfigNetwork => defmt::write!(fmt, "MissingConfigNetwork"),
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
