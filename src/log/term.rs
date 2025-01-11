use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};
use std::ops::Add;

//% Compliance:
//% `currentTerm` latest term server has seen (initialized to 0 on first boot, increases
//% monotonically)
const INITIAL_TERM: Term = Term(0);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Term(u64);

impl Term {
    pub const fn initial() -> Self {
        INITIAL_TERM
    }

    pub fn increment(&mut self) {
        self.0 += 1;
    }

    pub fn is_initial(&self) -> bool {
        *self == INITIAL_TERM
    }
}

impl Add<u64> for Term {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Term(self.0 + rhs)
    }
}

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
        encoder.encode(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    #[test]
    fn initial_value() {
        assert_eq!(INITIAL_TERM.0, 0);
    }

    #[test]
    fn encode_decode() {
        let term = Term::from(9);

        let mut slice = vec![0; 10];
        let mut buf = EncoderBuffer::new(&mut slice);
        term.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_term, _) = Term::decode(d_buf).unwrap();

        assert_eq!(term, d_term);
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
