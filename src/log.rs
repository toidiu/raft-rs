use core::{task::Context, time::Duration};
use tokio::time::sleep;

pub type DATA = u8;

pub struct Term(u64);
pub struct Idx(u64);

pub struct LogEntry {
    term: Term,
    idx: Idx,
    data: Vec<DATA>,
}
