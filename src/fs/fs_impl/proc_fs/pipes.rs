use alloc::{sync::{Arc, Weak}, collections::VecDeque};


pub struct PipeBuffer {
    inner: PipeBufferInner
}

pub struct PipeBufferInner {
    buffer: VecDeque<u8>
}

pub struct PipeWriteEnd {
    pub buffer: Arc<PipeBuffer>
}
pub struct PipeReadEnd {
    pub buffer: Weak<PipeBuffer>
}