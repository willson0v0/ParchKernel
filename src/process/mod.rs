mod pcb;
mod manager;
mod processor;
pub use pcb::{
    ProcessStatus,
    ProcessControlBlock,
    FileDescriptor
};

pub use manager::{
    enqueue,
    dequeue,
    ProcessID,
    new_pid
};

pub use processor::{
    push_intr_off,
    pop_intr_off,
    intr_off,
    intr_on,
    push_sum_on,
    pop_sum_on,
    get_processor,
    get_hart_id
};

pub fn init() {
    enqueue(ProcessControlBlock::new(crate::config::INIT_PROCESS.into()).unwrap());
}

pub fn hart_init() {
    get_processor().run();
}