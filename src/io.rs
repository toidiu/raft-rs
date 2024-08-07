use bytes::Bytes;

struct Buffer {
    data: Vec<Bytes>,
}

trait Io {
    fn recv() -> Buffer;

    fn send(buf: Buffer);
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        assert!(true);
    }
}
