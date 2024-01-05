use defmt::Formatter;

use crate::{codec, message};

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

impl<'a> defmt::Format for message::FromStrError<'a> {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "FromStrError::");
        match self {
            message::FromStrError::InvalidHex(e) => defmt::write!(fmt, "InvalidHex"),
            other @ _ => defmt::write!(fmt, "{:?}", other),
        }
    }
}

impl<'a> defmt::Format for message::SerialMessage<'a> {
    fn format(&self, fmt: Formatter<'_>) {
        defmt::write!(fmt, "SerialMessage::");
        match self {
            message::SerialMessage::SendData(message::SendData { data }) => defmt::write!(fmt, "SendData {{ data: {=[u8]:x} }}", data),
            message::SerialMessage::ReceiveData(message::ReceiveData { data }) => defmt::write!(fmt, "ReceiveData {{ data: {=[u8]:x} }}", data),
            message::SerialMessage::Configure(message::Configure { region, spreading_factor, network }) => defmt::write!(fmt, "Configure {{ region: {=u8}, spreading_factor: {=u8}, network: {=[u8]:x} }}", region, spreading_factor, network.as_ref()),
            message::SerialMessage::ReportRequest(_) => defmt::write!(fmt, "ReportRequest"),
            message::SerialMessage::Report(message::Report {
                sn,
                version_data,
                region,
                spreading_factor,
                receive_queue_size,
                transmit_queue_size,
                network,
            }) => defmt::write!(fmt, "Report {{ sn: {=u32}, version_data: {=[u8]:x}, region: {=u8}, spreading_factor: {=u8}, receive_queue_size: {=u8}, transmit_queue_size: {=u8}, network: {=[u8]:x} }}", sn, version_data.as_ref(), region, spreading_factor, receive_queue_size, transmit_queue_size, network.as_ref()),
            message::SerialMessage::Status(message::Status { code }) => defmt::write!(fmt, "Status({=?})", code),
            message::SerialMessage::UpgradeFirmwareRequest => defmt::write!(fmt, "UpgradeFirmwareRequest"),
            message::SerialMessage::SetTimestamp(message::SetTimestamp { timestamp }) => defmt::write!(fmt, "SetTimestamp({=u64})", timestamp),
            message::SerialMessage::GetRawIq => defmt::write!(fmt, "GetRawIq"),
            message::SerialMessage::RawIq(raw_iq) => defmt::write!(fmt, "RawIq {{ data: {=[u8]:x} }}", raw_iq.raw_data())
        }
    }
}
