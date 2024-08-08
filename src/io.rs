use bytes::Bytes;

pub trait IO {
    fn recv() -> Buffer;

    fn send(buf: Buffer);
}

struct Buffer {
    data: Vec<Bytes>,
}

pub struct BufferIO {
    in_buf: Buffer,
    out_buf: Buffer,
}

impl IO for BufferIO {
    fn recv() -> Buffer {
        todo!()
    }

    fn send(buf: Buffer) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {
        assert!(true);
    }
}
