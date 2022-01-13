use alloc::string::ToString;
use riscv::register::{
    sstatus,
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
use crate::config::MAX_CPUS;
use crate::utils::{Mutex, SpinMutex};

pub struct CPUManager {
    pub cpus: Vec<Arc<SpinMutex<CPU>>>
}

impl CPUManager {
    pub fn get_cpu(&self, hart: usize) -> Arc<SpinMutex<CPU>> {
        return self.cpus[hart].clone();
    }
}

/// this is because each hart only access it's corresponding CPU struct
unsafe impl Sync for CPUManager{}

lazy_static!{
    pub static ref CPU_MANAGER: CPUManager = {
        let mut cpus = Vec::new();
        for i in 0..MAX_CPUS {
            cpus.push(Arc::new(SpinMutex::new("CPU", CPU::new(i))))
        }
        CPUManager {
            cpus
        }
    };
}

/// Struct that repersent CPU's state
pub struct CPU {
    // TODO: CurrentPCB
    // TODO: Context for scheduler()
    int_off_count: usize,    // depth of push_off nesting
    int_enable_b4_off: bool,        // was interrupt enabled before push_off
    hart_id: usize
}

impl CPU {
    pub fn new(hart_id: usize) -> Self {
        Self {
            int_off_count: 0,
            int_enable_b4_off: false,
            hart_id
        }
    }

    /// WARN: Don't use these! use push/pop_intr_off!
    pub fn register_push_off(&mut self, intr_state_b4: bool) {
        if self.int_off_count == 0 {
            self.int_enable_b4_off = intr_state_b4;
        }
        self.int_off_count += 1;
    }

    pub fn register_pop_off(&mut self) -> bool {
        assert!(self.int_off_count >= 1, "unmatched pop_intr_off");
        self.int_off_count -= 1;
        self.int_off_count == 0 && self.int_enable_b4_off
    }

    pub fn intr_state(&self) -> bool{
        return sstatus::read().sie();
    }

    /// wut?
    pub fn get_id(&self) -> usize {
        let hart_id = get_hart_id();
        assert!(self.hart_id == hart_id, "bad cpu hart");
        hart_id
    }
}

pub fn get_hart_id() -> usize {
    let mut hart_id: usize;
    unsafe {
        asm! {
            "mv {0}, tp",
            out(reg) hart_id
        };
    }
    hart_id
}

pub fn get_cpu() -> Arc<SpinMutex<CPU>> {
    return CPU_MANAGER.get_cpu(get_hart_id());
}

pub fn push_intr_off() {
    // intr off, then lock
    let intr_state = sstatus::read().sie();
    unsafe {
        sstatus::clear_sie();
    }
    get_cpu().acquire_no_off().register_push_off(intr_state);
}

pub fn pop_intr_off() {
    // unlock, then intr on
    if get_cpu().acquire_no_off().register_pop_off() {
        unsafe {sstatus::set_sie()};
    }
}