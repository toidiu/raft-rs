pub type Data = u8;

pub struct Term(u64);
pub struct Idx(u64);

pub struct LogEntry {
    term: Term,
    idx: Idx,
    data: Vec<Data>,
}
