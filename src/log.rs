pub type Data = u8;

pub struct Term(u64);
pub struct Idx(u64);

pub struct LogEntry {
    term: Term,
    idx: Idx,
    data: Vec<Data>,
}

pub struct Log {
    entries: Vec<LogEntry>,
}

#[cfg(test)]
mod tests {

    #[test]
    fn log_test() {
        // - adding new entries to Log
        // - removing entries from Log
        // - comparing log entry
        // - get term,idx of last LogEntry
    }
}
