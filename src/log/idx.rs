use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

//% Compliance:
//% `commitIndex` index of highest log entry known to be committed (initialized to 0, increases
//% monotonically)
pub const INITIAL_IDX: Idx = Idx(0);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Idx(u64);

impl From<u64> for Idx {
    fn from(value: u64) -> Self {
        // index values should be greater than 0
        debug_assert!(value > 0);
        Idx(value)
    }
}

impl<'a> DecoderValue<'a> for Idx {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (idx, buffer): (u64, _) = buffer.decode()?;
        let idx = Idx(idx);
        Ok((idx, buffer))
    }
}

impl EncoderValue for Idx {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    #[test]
    fn initial_value() {
        assert_eq!(INITIAL_IDX.0, 0);
    }

    #[test]
    fn encode_decode() {
        let idx = Idx::from(9);

        let mut slice = vec![0; 10];
        let mut buf = EncoderBuffer::new(&mut slice);
        idx.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_idx, _) = Idx::decode(d_buf).unwrap();

        assert_eq!(idx, d_idx);
    }

    #[test]
    fn cmp_idx() {
        let idx = 4;
        let i = Idx(idx);
        let i_eq = Idx(idx);
        let i_lt = Idx(idx - 1);
        let i_gt = Idx(idx + 1);

        assert_eq!(i_eq, i);
        assert!(i_lt < i);
        assert!(i_gt > i);
    }
}
