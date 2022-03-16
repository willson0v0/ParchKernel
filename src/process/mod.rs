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
    process_list,
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
    pub static ref INIT_PROCESS: Arc<ProcessControlBlock> = {
        let init = ProcessControlBlock::new(crate::config::INIT_PROCESS_PATH.into()).unwrap();
        // let mut init_inner = init.get_inner();
        // let elf_file = init_inner.elf_file.clone();
        // (init_inner.entry_point, init_inner.data_end) = init_inner.mem_layout.map_elf(elf_file).unwrap();
        // drop(init_inner);
        init
    };
}

pub fn init() {
    enqueue(INIT_PROCESS.clone());
    milestone!("Init_process initialzed and enqueued for execution.");
}

pub fn hart_init() {
    milestone!("Starting scheduler on hart {}...", get_hart_id());
    get_processor().run();
}