use s2n_codec::{DecoderValue, EncoderValue};

macro_rules! id {
    ($name: ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
        pub struct $name([u8; 16]);

        impl $name {
            pub fn new(id: [u8; 16]) -> Self {
                $name(id)
            }
        }

        impl<'a> DecoderValue<'a> for $name {
            fn decode(
                buffer: s2n_codec::DecoderBuffer<'a>,
            ) -> s2n_codec::DecoderBufferResult<'a, Self> {
                let (candidate_id, buffer) = buffer.decode_slice(16)?;
                let candidate_id = candidate_id
                    .into_less_safe_slice()
                    .try_into()
                    .expect("failed to decode $name");
                Ok(($name(candidate_id), buffer))
            }
        }

        impl EncoderValue for $name {
            fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
                encoder.write_slice(&self.0)
            }
        }
    };
}

id!(ServerId);
// TODO: add PeerId for type safety
// id!(PeerId);

#[cfg(test)]
mod tests {
    use super::*;
    use s2n_codec::{DecoderBuffer, DecoderValue, EncoderBuffer, EncoderValue};

    #[test]
    fn encode_decode() {
        let id = ServerId::new([5; 16]);

        let mut slice = vec![0; 20];
        let mut buf = EncoderBuffer::new(&mut slice);
        id.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_id, _) = ServerId::decode(d_buf).unwrap();

        assert_eq!(id, d_id);
    }

    #[test]
    fn cmp_test() {
        let id1 = ServerId::new([10; 16]);
        let mut id2 = ServerId::new([10; 16]);

        assert_eq!(id1, id2);

        id2.0[1] = 11;
        assert_ne!(id1, id2);
    }
}
