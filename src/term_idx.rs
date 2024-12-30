use core::cmp::Ordering;
use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct TermIdx {
    term: Term,
    idx: Idx,
}

impl TermIdx {
    pub fn builder() -> TermIdxBuilder {
        TermIdxBuilder::default()
    }

    pub fn term(&self) -> Term {
        self.term
    }

    pub fn idx(&self) -> Idx {
        self.idx
    }
}

#[derive(Default)]
pub(crate) struct TermIdxBuilder {
    term: Option<u64>,
    idx: Option<u64>,
}

impl TermIdxBuilder {
    fn with_term(mut self, term: u64) -> Self {
        self.term = Some(term);
        self
    }

    fn with_idx(mut self, idx: u64) -> Self {
        self.idx = Some(idx);
        self
    }

    fn build(self) -> Result<TermIdx, ()> {
        let term = self.term.ok_or(())?;
        let idx = self.idx.ok_or(())?;

        Ok(TermIdx {
            term: Term(term),
            idx: Idx(idx),
        })
    }

    fn build_unchecked(self) -> TermIdx {
        self.build().unwrap()
    }
}

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
//% Compliance:
//% initialized to 0, increases monotonically
pub(crate) struct Term(pub u64);

#[derive(Default, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
//% Compliance:
//% initialized to 0, increases monotonically
pub(crate) struct Idx(pub u64);

impl PartialOrd for TermIdx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TermIdx {
    fn cmp(&self, other: &Self) -> Ordering {
        let term_ord = self.term.cmp(&other.term);
        let idx_ord = self.idx.cmp(&other.idx);

        //% Compliance:
        //% up-to-date: a log is considered more up-to-date than another log if:
        //%   compare the index and term of the last entry of A's and B's log
        //%   if the entries have different term: the higher term is more up-to-date
        //%   if the term is the same: the longer log (higher index) is more up-to-date
        match (term_ord, idx_ord) {
            (Ordering::Less, _) => Ordering::Less,
            (Ordering::Greater, _) => Ordering::Greater,
            (Ordering::Equal, Ordering::Less) => Ordering::Less,
            (Ordering::Equal, Ordering::Equal) => Ordering::Equal,
            (Ordering::Equal, Ordering::Greater) => Ordering::Greater,
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
    fn initial_value() {
        let idx = Idx::default();
        assert_eq!(idx.0, 0);

        let term = Term::default();
        assert_eq!(term.0, 0);
    }

    #[test]
    fn cmp_term_idx() {
        let term = 2;
        let idx = 4;

        let ti1 = TermIdx::builder()
            .with_term(term)
            .with_idx(idx)
            .build_unchecked();

        // equal
        let ti_equal = TermIdx::builder()
            .with_term(term)
            .with_idx(idx)
            .build_unchecked();
        assert_eq!(ti1, ti_equal);

        // term difference
        let ti_less = TermIdx::builder()
            .with_term(term - 1)
            .with_idx(idx)
            .build_unchecked();
        assert!(ti_less < ti1);
        let ti_greater = TermIdx::builder()
            .with_term(term + 1)
            .with_idx(idx)
            .build_unchecked();
        assert!(ti_greater > ti1);

        // idx difference
        let ti_less = TermIdx::builder()
            .with_term(term)
            .with_idx(idx - 1)
            .build_unchecked();
        assert!(ti_less < ti1);
        let ti_greater = TermIdx::builder()
            .with_term(term)
            .with_idx(idx + 1)
            .build_unchecked();
        assert!(ti_greater > ti1);
    }
}
