use crate::log::term::Term;
use s2n_codec::{DecoderBufferResult, DecoderValue, EncoderValue};

type Command = u8;

//% Compliance:
//% each log entry stores
//% 	- state machine command
//% 	- term number
//% 	- log index: integer
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub term: Term,
    pub command: Command,
}

impl Entry {
    pub fn new(term: Term, command: Command) -> Self {
        Entry { term, command }
    }
}

impl<'a> DecoderValue<'a> for Entry {
    fn decode(buffer: s2n_codec::DecoderBuffer<'a>) -> DecoderBufferResult<'a, Self> {
        let (term_idx, buffer) = buffer.decode()?;
        let (command, buffer) = buffer.decode()?;

        let entry = Entry {
            term: term_idx,
            command,
        };
        Ok((entry, buffer))
    }
}

impl EncoderValue for Entry {
    fn encode<E: s2n_codec::Encoder>(&self, encoder: &mut E) {
        encoder.encode(&self.term);
        encoder.encode(&self.command);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use s2n_codec::{DecoderBuffer, EncoderBuffer};

    #[test]
    fn encode_decode() {
        let entry = Entry::new(Term::from(2), 5);

        let mut slice = vec![0; 30];
        let mut buf = EncoderBuffer::new(&mut slice);
        entry.encode(&mut buf);

        let d_buf = DecoderBuffer::new(&slice);
        let (d_entry, _) = Entry::decode(d_buf).unwrap();

        assert_eq!(entry, d_entry);
    }
}
