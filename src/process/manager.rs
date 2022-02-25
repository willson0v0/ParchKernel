use core::sync::atomic::{AtomicU64, Ordering};

use alloc::{collections::VecDeque, sync::Arc};
use lazy_static::*;

use crate::utils::{SpinMutex, MutexGuard, Mutex, ErrorNum};

use super::ProcessControlBlock;

lazy_static!{
    static ref PROCESS_MANAGER: ProcessManager = ProcessManager::new();
    static ref PID_ALLOCATOR: PIDAllocator = PIDAllocator::new();
}

struct ProcessManager(SpinMutex<ProcessManagerInner>);

impl ProcessManager {
    pub fn new() -> Self {
        verbose!("Initializing ProcessManager");
        Self(SpinMutex::new("ProcessManager", ProcessManagerInner::new()))
    }

    pub fn inner_locked(&self) -> MutexGuard<ProcessManagerInner> {
        self.0.acquire()
    }
}

struct ProcessManagerInner{
    process_list: VecDeque<Arc<ProcessControlBlock>>
}

impl ProcessManagerInner {
    pub fn new() -> Self {
        Self {
            process_list: VecDeque::new()
        }
    }

    pub fn enqueue(&mut self, process: Arc<ProcessControlBlock>) {
        self.process_list.push_back(process);
    }

    pub fn dequeue(&mut self) -> Option<Arc<ProcessControlBlock>> {
        self.process_list.pop_back()
    }
}

pub fn enqueue(process: Arc<ProcessControlBlock>) {
    PROCESS_MANAGER.inner_locked().enqueue(process);
}

pub fn dequeue() -> Option<Arc<ProcessControlBlock>> {
    PROCESS_MANAGER.inner_locked().dequeue()
}

pub struct ProcessID(u64);

/// NEVER REUSE PID
struct PIDAllocator(AtomicU64);

impl PIDAllocator {
    pub fn new() -> Self {
        Self (AtomicU64::new(0))
    }

    pub fn next(&self) -> ProcessID {
        ProcessID(self.0.fetch_add(1, Ordering::SeqCst))
    }
}

pub fn new_pid() -> ProcessID {
    return PID_ALLOCATOR.next();
}