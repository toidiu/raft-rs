use crate::{rpc::Rpc, server::ServerId};
use s2n_codec::{DecoderBuffer, DecoderBufferResult, DecoderValue, EncoderValue};

#[derive(Debug, PartialEq, Eq)]
pub struct Packet {
    header: Header,
    rpc: Rpc,
}

impl Packet {
    pub fn new(header: Header, rpc: Rpc) -> Packet {
        Packet { header, rpc }
    }

    pub fn rpc(&self) -> &Rpc {
        &self.rpc
    }

    pub fn from(&self) -> ServerId {
        self.header.from
    }

    pub fn to(&self) -> ServerId {
        self.header.to
    }
}

impl<'a> DecoderValue<'a> for Packet {
    fn decode(buffer: DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (header, buffer) = buffer.decode()?;
        let (rpc, buffer) = buffer.decode()?;

        Ok((Packet { header, rpc }, buffer))
    }
}

impl EncoderValue for Packet {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.header);
        encoder.encode(&self.rpc);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Header {
    pub(crate) from: ServerId,
    pub(crate) to: ServerId,
}

impl<'a> DecoderValue<'a> for Header {
    fn decode(buffer: DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (from, buffer) = buffer.decode()?;
        let (to, buffer) = buffer.decode()?;

        Ok((Header { from, to }, buffer))
    }
}

impl EncoderValue for Header {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.from);
        encoder.encode(&self.to);
    }
}
