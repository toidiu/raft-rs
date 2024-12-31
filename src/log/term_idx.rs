use crate::log::{idx::Idx, term::Term};
use core::cmp::Ordering;
use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct TermIdx {
    term: Term,
    idx: Idx,
}

impl TermIdx {
    pub fn term(&self) -> Term {
        self.term
    }

    pub fn idx(&self) -> Idx {
        self.idx
    }
}

impl PartialOrd for TermIdx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TermIdx {
    fn cmp(&self, other: &Self) -> Ordering {
        let term_ord = self.term.cmp(&other.term);
        let idx_ord = self.idx.cmp(&other.idx);

        match (term_ord, idx_ord) {
            (Ordering::Less, _) => Ordering::Less,
            (Ordering::Greater, _) => Ordering::Greater,
            (Ordering::Equal, idx_ord) => idx_ord,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmp_term_idx() {
        let term = 2;
        let idx = 4;

        let term_idx = TermIdx {
            term: Term::from(term),
            idx: Idx::from(idx),
        };

        // equal
        let eq = TermIdx {
            term: Term::from(term),
            idx: Idx::from(idx),
        };
        assert_eq!(term_idx, eq);

        // term compare
        let term_lt = TermIdx {
            term: Term::from(term - 1),
            idx: Idx::from(idx),
        };
        let term_gt = TermIdx {
            term: Term::from(term + 1),
            idx: Idx::from(idx),
        };
        assert!(term_lt < term_idx);
        assert!(term_gt > term_idx);

        // idx compare
        let idx_lt = TermIdx {
            term: Term::from(term),
            idx: Idx::from(idx - 1),
        };
        let idx_gt = TermIdx {
            term: Term::from(term),
            idx: Idx::from(idx + 1),
        };
        assert!(idx_lt < term_idx);
        assert!(idx_gt > term_idx);
    }
}
