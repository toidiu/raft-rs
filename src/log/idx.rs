use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

//% Compliance:
//% initialized to 0, increases monotonically
const INITIAL_IDX: Idx = Idx(0);

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) struct Idx(u64);

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
        encoder.write_slice(&self.0.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_value() {
        let idx = Idx::default();
        assert_eq!(idx.0, 0);
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