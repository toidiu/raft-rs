//! A byte queue that never grows past a ceiling fixed at construction.

use std::collections::VecDeque;

/// A byte queue with a fixed ceiling.
///
/// `VecDeque` is already a ring buffer but it reallocates when it fills, so its `capacity` tracks
/// the allocation rather than any policy. Holding the ceiling here instead makes it a real limit,
/// and keeps the one subtraction that computes free space next to the only code that can consume
/// it.
///
/// Without a ceiling a server that outruns the network task queues packets until the process dies.
/// Refusing a write sheds that load in the one way Raft already handles, which is a lost packet.
#[derive(Debug)]
pub struct BoundedQueue {
    bytes: VecDeque<u8>,

    // Fixed at construction. `bytes.capacity()` is not a substitute since it grows on demand.
    capacity: usize,
}

impl BoundedQueue {
    pub fn new(capacity: usize) -> Self {
        BoundedQueue {
            bytes: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    /// Bytes that can still be written.
    ///
    /// Cannot underflow. [`Self::try_extend`] is the only writer and it refuses anything that
    /// would take the length past the ceiling.
    pub fn remaining(&self) -> usize {
        self.capacity - self.bytes.len()
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Append every byte or none of them. False means the data did not fit.
    ///
    /// All or nothing because the queue is a byte stream with no record boundaries.
    pub fn try_extend(&mut self, data: &[u8]) -> bool {
        if data.len() > self.remaining() {
            return false;
        }

        self.bytes.extend(data);
        true
    }

    /// Move up to `dst.len()` bytes out of the queue and return how many moved.
    ///
    /// Takes whatever is there rather than waiting for a whole packet, since the queue has no way
    /// to know where a packet ends. The caller is what reassembles them.
    ///
    /// Deliberately not `std::io::Read`, whose
    /// impl for `VecDeque` copies only the front of the ring and leaves the rest for a later call.
    pub fn read_into(&mut self, dst: &mut [u8]) -> usize {
        let len = dst.len().min(self.bytes.len());

        // A wrapped ring holds its contents as two regions, front first in write order.
        let (front, back) = self.bytes.as_slices();
        let split = len.min(front.len());

        dst[..split].copy_from_slice(&front[..split]);
        dst[split..len].copy_from_slice(&back[..len - split]);

        self.bytes.drain(..len);
        len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A write that would cross the ceiling is refused whole and changes nothing.
    ///
    /// Fills a 4 byte queue to 3, then offers 2 more bytes.
    ///
    /// Catches the failure that motivates the type. Appending the one byte that fits would leave a
    /// truncated packet in the stream, and a reader has no way to distinguish that from a packet
    /// still arriving, so every packet behind it decodes as garbage.
    #[test]
    fn refuses_a_write_that_does_not_fit() {
        let mut queue = BoundedQueue::new(4);
        assert!(queue.try_extend(&[1, 2, 3]));
        assert_eq!(queue.remaining(), 1);

        // Two bytes into one byte of room.
        assert!(!queue.try_extend(&[4, 5]));

        // The refused write left no trace.
        assert_eq!(queue.len(), 3);
        assert_eq!(queue.remaining(), 1);

        // A write that does fit still succeeds afterwards, so a refusal does not wedge the queue.
        assert!(queue.try_extend(&[6]));
        assert_eq!(queue.remaining(), 0);
    }

    /// One read drains a wrapped queue completely instead of stopping at the wrap.
    ///
    /// Fills a 4 byte queue, drains half so the head advances, then refills so the contents
    /// straddle the end of the allocation. Reads it all back through a destination with room to
    /// spare, and checks first that the ring really did wrap.
    ///
    /// `std::io::Read for VecDeque` copies only the front region and reports a short count. A
    /// caller cannot tell that from a genuinely partial read, so it decodes what it got and cuts
    /// every packet straddling the wrap in half. The oversized destination is what exposes it. A
    /// destination sized to the contents passes either way.
    #[test]
    fn one_read_drains_across_the_wrap() {
        let mut queue = BoundedQueue::new(4);
        assert!(queue.try_extend(&[1, 2, 3, 4]));
        assert_eq!(queue.remaining(), 0);

        // Drain half, which moves the head and leaves room at the front of the ring.
        let mut dst = [0; 2];
        assert_eq!(queue.read_into(&mut dst), 2);
        assert_eq!(dst, [1, 2]);
        assert_eq!(queue.remaining(), 2);

        // Refill so the contents straddle the end of the allocation.
        assert!(queue.try_extend(&[5, 6]));

        // Guard the premise. Without two non-empty regions this test proves nothing, and a future
        // capacity change could quietly make the contents contiguous again.
        let (front, back) = queue.bytes.as_slices();
        assert!(
            !front.is_empty() && !back.is_empty(),
            "expected a wrapped ring, got front {front:?} back {back:?}"
        );

        // Room to spare, so a count below 4 means bytes were left behind rather than no space.
        let mut dst = [0; 8];
        assert_eq!(queue.read_into(&mut dst), 4);
        assert_eq!(&dst[..4], &[3, 4, 5, 6]);
        assert!(queue.is_empty());
    }

    /// Reading an empty queue reports zero rather than blocking or failing.
    ///
    /// Reads from a queue nothing was ever written to.
    ///
    /// Callers use the count to decide whether anything arrived. A read that returned an error or
    /// a nonzero count here would make an idle server act as though a packet were waiting.
    #[test]
    fn reading_an_empty_queue_returns_zero() {
        let mut queue = BoundedQueue::new(4);

        let mut dst = [0; 2];
        assert_eq!(queue.read_into(&mut dst), 0);
        assert_eq!(dst, [0, 0]);
    }
}
