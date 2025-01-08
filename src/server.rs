use crate::{mode::Mode, peer::Peer, state::State};
use s2n_codec::{DecoderValue, EncoderValue};

struct Server {
    mode: Mode,
    state: State,
    // FIXME make into Set
    peer_list: Vec<Peer>,
}

pub struct Context<'a> {
    pub server_id: ServerId,
    pub state: &'a mut State,
    // FIXME make into Set
    pub peer_list: &'a Vec<Peer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ServerId([u8; 16]);

impl ServerId {
    pub fn new(id: [u8; 16]) -> Self {
        ServerId(id)
    }
}

impl<'a> DecoderValue<'a> for ServerId {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> s2n_codec::DecoderBufferResult<'a, Self> {
        let (candidate_id, buffer) = buffer.decode_slice(16)?;
        let candidate_id = candidate_id
            .into_less_safe_slice()
            .try_into()
            .expect("failed to decode ServerId");
        Ok((ServerId(candidate_id), buffer))
    }
}

impl EncoderValue for ServerId {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.write_slice(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    #[test]
    fn encode_decode() {
        let id = ServerId([5; 16]);

        let mut slice = vec![0; 20];
        let mut buf = EncoderBuffer::new(&mut slice);
        id.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_id, _) = ServerId::decode(d_buf).unwrap();

        assert_eq!(id, d_id);
    }

    #[test]
    fn cmp_test() {
        let id1 = ServerId([10; 16]);
        let mut id2 = ServerId([10; 16]);

        assert_eq!(id1, id2);

        id2.0[1] = 11;
        assert_ne!(id1, id2);
    }
}
