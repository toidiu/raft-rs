use core::{cmp::Ordering, ops::AddAssign};
use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct TermIdx {
    term: Term,
    idx: Idx,
}

impl TermIdx {
    pub fn new(term: u64, idx: u64) -> Self {
        debug_assert!(idx > 0);
        TermIdx {
            term: Term(term),
            idx: Idx(idx),
        }
    }

    pub fn term(&self) -> Term {
        self.term
    }

    pub fn idx(&self) -> Idx {
        self.idx
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) struct Term(pub u64);

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) struct Idx(pub u64);

impl PartialOrd for TermIdx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TermIdx {
    fn cmp(&self, other: &Self) -> Ordering {
        // compare term
        let term = self.term.cmp(&other.term);

        // compare idx
        let idx = self.idx.cmp(&other.idx);

        match (term, idx) {
            (Ordering::Less, _) => Ordering::Less,
            (Ordering::Equal, Ordering::Less) => Ordering::Less,
            (Ordering::Equal, Ordering::Equal) => Ordering::Equal,
            (Ordering::Equal, Ordering::Greater) => Ordering::Greater,
            (Ordering::Greater, _) => Ordering::Greater,
        }
    }
}

impl<'a> DecoderValue<'a> for TermIdx {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (term, buffer): (Term, _) = buffer.decode()?;
        let (idx, buffer): (Idx, _) = buffer.decode()?;
        Ok((TermIdx { term, idx }, buffer))
    }
}

impl EncoderValue for TermIdx {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.encode(&self.idx);
    }
}

impl AddAssign<u64> for Term {
    fn add_assign(&mut self, rhs: u64) {
        self.0 += rhs;
    }
}

impl From<u64> for Term {
    fn from(value: u64) -> Self {
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
    fn new_term_idx() {
        let ti1 = TermIdx::new(2, 4);

        let ti_equal = TermIdx::new(2, 4);
        assert!(ti1 == ti_equal);

        let mut ti_less = TermIdx::new(0, 4);
        assert!(ti_less < ti1);
        ti_less = TermIdx::new(1, 3);
        assert!(ti_less < ti1);

        let mut ti_greater = TermIdx::new(3, 4);
        assert!(ti_greater > ti1);
        ti_greater = TermIdx::new(2, 5);
        assert!(ti_greater > ti1);
    }
}
