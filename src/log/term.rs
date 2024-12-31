use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

//% Compliance:
//% initialized to 0, increases monotonically
const INITIAL_TERM: Term = Term(0);

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) struct Term(u64);

impl From<u64> for Term {
    fn from(value: u64) -> Self {
        // term values should be greater than 0
        debug_assert!(value > 0);
        Term(value)
    }
}

impl<'a> DecoderValue<'a> for Term {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (term, buffer): (u64, _) = buffer.decode()?;
        let term = Term(term);
        Ok((term, buffer))
    }
}

impl EncoderValue for Term {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.write_slice(&self.0.to_be_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_value() {
        let term = Term::default();
        assert_eq!(term.0, 0);
    }

    #[test]
    fn cmp_term() {
        let term = 4;

        let i = Term(term);
        let i_eq = Term(term);
        let i_lt = Term(term - 1);
        let i_gt = Term(term + 1);

        assert_eq!(i_eq, i);
        assert!(i_lt < i);
        assert!(i_gt > i);
    }
}