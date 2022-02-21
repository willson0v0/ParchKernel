use crate::{mem::MemLayout, utils::{SpinMutex, MutexGuard, Mutex}};



pub struct ProcessControlBlock {
    inner: SpinMutex<PCBInner>
}

pub struct PCBInner {
    pub mem_layout: MemLayout
}

impl ProcessControlBlock {
    pub fn get_inner(&self) -> MutexGuard<PCBInner> {
        self.inner.acquire()
    }
}