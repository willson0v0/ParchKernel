use core::sync::atomic::{Ordering, AtomicUsize};

use alloc::{collections::{VecDeque}, sync::{Arc, Weak}, vec::Vec};
use lazy_static::*;

use crate::{utils::{SpinMutex, MutexGuard, Mutex, ErrorNum}, config::MAX_CPUS};

use super::{ProcessControlBlock, get_hart_id};

lazy_static!{
    static ref PROCESS_MANAGER: ProcessManager = ProcessManager::new();
    static ref PID_ALLOCATOR: PIDAllocator = PIDAllocator::new();
}

// TODO: lock free queue
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
    pub process_list: VecDeque<Arc<ProcessControlBlock>>,
    pub running_list: [Option<Weak<ProcessControlBlock>>; MAX_CPUS]
}

impl ProcessManagerInner {
    pub fn new() -> Self {
        Self {
            process_list: VecDeque::new(),
            running_list: Default::default(),
        }
    }

    pub fn enqueue(&mut self, process: Arc<ProcessControlBlock>) {
        self.running_list[get_hart_id()].take();
        self.process_list.push_back(process);
    }

    /// guard by mutex, intr off, get_hart_id safe.
    pub fn dequeue(&mut self) -> Option<Arc<ProcessControlBlock>> {
        if let Some(proc ) = self.process_list.pop_front() {
            self.running_list[get_hart_id()] = Some(Arc::downgrade(&proc));
            Some(proc)
        } else {
            None
        }
    }

    pub fn free_current(&mut self) {
        self.running_list[get_hart_id()].take().expect("No process is running.");
    }

    pub fn get_process(&self, pid: ProcessID) -> Result<Arc<ProcessControlBlock>, ErrorNum> {
        for proc in self.process_list.iter() {
            if proc.pid == pid {
                return Ok(proc.clone());
            }
        }
        for proc in self.running_list.iter() {
            if let Some(proc) = proc {
                if let Some(proc) = proc.upgrade() {
                    if proc.pid == pid {
                        return Ok(proc.clone());
                    }
                }
            }
        }
        Err(ErrorNum::ESRCH)
    }

    pub fn enumerate_process(&self) -> Vec<Arc<ProcessControlBlock>> {
        let mut res: Vec<Arc<ProcessControlBlock>> = self.process_list.clone().into();
        for p in self.running_list.iter() {
            if let Some(v) = p.clone() {
                if let Some(v) = v.upgrade() {
                    res.push(v)
                }
            }
        }
        res
    }
}

pub fn enqueue(process: Arc<ProcessControlBlock>) {
    PROCESS_MANAGER.inner_locked().enqueue(process);
}

pub fn dequeue() -> Option<Arc<ProcessControlBlock>> {
    PROCESS_MANAGER.inner_locked().dequeue()
}

pub fn free_current() {
    PROCESS_MANAGER.inner_locked().free_current();
}

pub fn get_process(pid: ProcessID) -> Result<Arc<ProcessControlBlock>, ErrorNum> {
    PROCESS_MANAGER.inner_locked().get_process(pid)
}

pub fn process_list() -> Vec<Arc<ProcessControlBlock>> {
    PROCESS_MANAGER.inner_locked().enumerate_process()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessID(pub usize);

impl From<usize> for ProcessID {
    fn from(pid: usize) -> Self {
        Self (pid)
    }
}


impl core::fmt::Display for ProcessID {
	fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
		core::fmt::Debug::fmt(self, f)
	}
}

/// NEVER REUSE PID
struct PIDAllocator(AtomicUsize);

impl PIDAllocator {
    /// start from 1, for 0 is for shecduler kernel thread
    pub fn new() -> Self {
        Self (AtomicUsize::new(1))
    }

    pub fn next(&self) -> ProcessID {
        ProcessID(self.0.fetch_add(1, Ordering::SeqCst))
    }
}

pub fn new_pid() -> ProcessID {
    return PID_ALLOCATOR.next();
}