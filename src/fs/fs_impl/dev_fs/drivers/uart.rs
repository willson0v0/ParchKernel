//! alloc-less standalone uart

use crate::utils::SpinMutex;

struct RingBuffer<const N: usize>{
    inner: SpinMutex<RingBufferInner<N>>
}

struct RingBufferInner<const N: usize> {
    buf: [u8; N],
    head: usize,
    tail: usize
}

impl<const N: usize> RingBufferInner<N> {
    pub fn new() -> Self {
        Self {
            buf: [0u8; N],
            head: 0,
            tail: 0
        }
    }

    fn wrap_around(pos: usize) -> usize {
        pos % N
    }

    pub fn len(&self) -> usize {
        if self.head <= self.tail {
            self.tail - self.head
        } else {
            self.tail + N - self.head
        }
    }

    pub fn push(&mut self, mut buf: &[u8]) -> usize {
        // truncate
        buf = &buf[0..self.len()];

        let end = self.tail + buf.len();
        if end > self.len() {
            // wrap around
            let p1 = &buf[..(N-self.tail)];
            let p2 = &buf[(N-self.tail)..];
            self.buf[self.tail..].copy_from_slice(p1);
            self.buf[..p2.len()].copy_from_slice(p2);
        } else {
            self.buf[self.tail..(self.tail + buf.len())].copy_from_slice(buf);
        }
        self.tail = Self::wrap_around(self.tail + buf.len());
        buf.len()
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.len() > 0 {
            let result = self.buf[self.head];
            self.head += 1;
            Some(result)
        } else {
            None
        }
    }

    pub fn pop_x(&mut self, mut buf: &mut [u8]) -> usize {
        // truncate
        buf = &mut buf[..self.len()];

        if self.head <= self.tail && buf.len() + self.head > N {
            // wrap around?
            let p1 = &self.buf[self.head..];
            let p2 = &self.buf[..(buf.len() - p1.len())];
            buf[..p1.len()].copy_from_slice(p1);
            buf[p1.len()..].copy_from_slice(p2);
        } else {
            buf.copy_from_slice(&self.buf[self.head..(self.head + buf.len())]);
        }
        self.head = Self::wrap_around(self.head + buf.len());
        buf.len()
    }
}