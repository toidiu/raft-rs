use crate::{
    io::ServerIO,
    log::{idx::Idx, term::Term},
    peer::Peer,
    state::State,
};
use core::cmp::Ordering;
use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

const INITIAL_TERM_IDX: TermIdx = TermIdx {
    term: Term::initial(),
    idx: Idx::initial(),
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TermIdx {
    pub term: Term,
    pub idx: Idx,
}

impl TermIdx {
    pub fn builder() -> TermIdxWithTermBuilder {
        TermIdxWithTermBuilder
    }

    pub const fn initial() -> Self {
        INITIAL_TERM_IDX
    }

    pub fn is_initial(&self) -> bool {
        *self == INITIAL_TERM_IDX
    }

    pub fn prev_term_idx<T: ServerIO>(peer: &Peer<T>, state: &State) -> TermIdx {
        let next_log_idx = state.next_idx.get(&peer.id).unwrap();
        if *next_log_idx == Idx::from(1) {
            // peer's log is empty
            TermIdx::initial()
        } else {
            let prev_log_idx = Idx::from(next_log_idx.log_idx_value() as u64);
            let prev_log_term = state.log.last_term();
            TermIdx::builder()
                .with_term(prev_log_term)
                .with_idx(prev_log_idx)
        }
    }
}

#[derive(Debug)]
pub struct TermIdxWithTermBuilder;

impl TermIdxWithTermBuilder {
    pub fn with_term(self, term: Term) -> TermIdxWithIdxBuilder {
        TermIdxWithIdxBuilder {
            term,
            idx: Idx::initial(),
        }
    }
}

#[derive(Debug)]
pub struct TermIdxWithIdxBuilder {
    term: Term,
    idx: Idx,
}

impl TermIdxWithIdxBuilder {
    pub fn with_idx(self, idx: Idx) -> TermIdx {
        TermIdx {
            term: self.term,
            idx,
        }
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

        //% Compliance:
        //% `up-to-date`: a log is considered more up-to-date than another log if:
        //% 	- compare the index and term of the last entry of A's and B's log
        //% 	- if the entries have different term: the higher term is more up-to-date
        //% 	- if the term is the same: the longer log (higher index) is more up-to-date
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
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    #[test]
    fn encode_decode() {
        let term = Term::from(9);
        let idx = Idx::from(9);
        let term_idx = TermIdx { term, idx };

        let mut slice = vec![0; 20];
        let mut buf = EncoderBuffer::new(&mut slice);
        term_idx.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_term_idx, _) = TermIdx::decode(d_buf).unwrap();

        assert_eq!(term_idx, d_term_idx);
    }

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
