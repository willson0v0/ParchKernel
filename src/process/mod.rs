mod pcb;
mod manager;
mod processor;
use alloc::sync::Arc;
pub use pcb::{
    ProcessStatus,
    ProcessControlBlock,
    FileDescriptor
};
pub mod def_handler;
mod signal_num;

pub use signal_num::SignalNum;

pub use manager::{
    enqueue,
    dequeue,
    ProcessID,
    new_pid,
    get_process,
    free_current
};

pub use processor::{
    push_intr_off,
    pop_intr_off,
    intr_off,
    intr_on,
    push_sum_on,
    pop_sum_on,
    get_processor,
    get_hart_id,
    PROCESSOR_MANAGER
};

use lazy_static::*;
lazy_static!{
    pub static ref INIT_PROCESS: Arc<ProcessControlBlock> = ProcessControlBlock::new(crate::config::INIT_PROCESS_PATH.into()).unwrap();
}

pub fn init() {
    enqueue(INIT_PROCESS.clone());
    milestone!("Init_process initialzed and enqueued for execution.");
}

pub fn hart_init() {
    milestone!("Starting scheduler on hart {}...", get_hart_id());
    get_processor().run();
}