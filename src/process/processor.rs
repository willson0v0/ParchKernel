use core::arch::{asm, global_asm};
use core::cell::RefCell;


use riscv::register::{
    sstatus, satp,
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
use crate::config::{MAX_CPUS, PROC_K_STACK_ADDR, PROC_K_STACK_SIZE};
use crate::interrupt::{trap_return, fork_return};
use crate::mem::{MemLayout};
use crate::process::ProcessControlBlock;
use crate::process::pcb::ProcessStatus;
use crate::utils::{Mutex, MutexGuard};

use super::pcb::PCBInner;
use super::{dequeue, enqueue, INIT_PROCESS};

global_asm!(include_str!("swtch.asm"));

extern "C" {
    /// The `__swtch()` function for switching kernel execution flow.
    pub fn __swtch(
        current_context: *mut ProcessContext,
        next_context: *const ProcessContext
    );
}

/// The process context used in `__switch` (kernel execution flow) 
/// Saved on top of the kernel stack of corresponding process.
#[repr(C)]
#[derive(Debug)]
pub struct ProcessContext {
    ra      : usize,
    sp      : usize,
    s_regs  : [usize; 12],
    s_fregs : [f64; 12]
}

impl ProcessContext {
    pub fn new() -> Self {
        Self {
            ra: fork_return as usize,
            sp: PROC_K_STACK_ADDR.0 + PROC_K_STACK_SIZE,    // Stack top
            s_regs: [0; 12],
            s_fregs:[0.0; 12]
        }
    }
}

pub struct ProcessorManager {
    processor_list: Vec<Arc<Processor>>
}

impl ProcessorManager {
    pub fn new(processor_list: Vec<Arc<Processor>>) -> Self{
        Self {processor_list}
    }

    pub fn get_processor(&self, hart: usize) -> Arc<Processor> {
        assert!(get_hart_id() == hart, "CPUManager Access violation");
        self.processor_list[hart].clone()
    }
}

/// this is because each hart only access it's corresponding CPU struct
unsafe impl Sync for ProcessorManager{}

lazy_static!{
    pub static ref PROCESSOR_MANAGER: ProcessorManager = {
        let mut cpus = Vec::new();
        for i in 0..MAX_CPUS {
            cpus.push(Arc::new(Processor::new(i)))
        }
        ProcessorManager::new(cpus)
    };
}

/// Struct that repersent CPU's state
pub struct Processor {
    pub hart_id: usize,
    inner: RefCell<ProcessorInner>
}

unsafe impl Sync for Processor{}

pub struct ProcessorInner {
    pub pcb: Option<Arc<ProcessControlBlock>>,
    pub int_off_count: usize,    // depth of push_off nesting
    pub int_enable_b4_off: bool,        // was interrupt enabled before push_off
    pub sum_count: usize,
    pub idle_context: ProcessContext,
    pub sche_mem_layout: Option<MemLayout>
}

impl Processor {
    pub fn new(hart_id: usize) -> Self {
        Self {
            hart_id,
            inner: RefCell::new(ProcessorInner::new())
        }
    }

    /// WARN: Don't use these! use push/pop_intr_off!
    pub fn register_push_off(&self, intr_state_b4: bool) {
        // self.inner.borrow_mut().register_push_off(intr_state_b4);
        let res =  self.inner.try_borrow_mut();
        match res {
            Ok(mut inner) => {inner.register_push_off(intr_state_b4)},
            Err(err) => {panic!("wtf @ {:?}", err)}
        }
    }

    pub fn register_pop_off(&self) -> bool {
        self.inner.borrow_mut().register_pop_off()
    }

    pub fn intr_state(&self) -> bool{
        return sstatus::read().sie();
    }

    pub fn get_context(&self) -> *mut ProcessContext {
        self.inner.borrow_mut().get_context()
    }
    
    pub fn push_sum_on(&self) {
        self.inner.borrow_mut().push_sum_on();
    }

    pub fn pop_sum_on(&self) {
        self.inner.borrow_mut().pop_sum_on();
    }

    pub fn current(&self) -> Option<Arc<ProcessControlBlock>> {
        self.inner.borrow().pcb.clone()
    }

    pub fn take_current(&self) -> Option<Arc<ProcessControlBlock>> {
        self.inner.borrow_mut().pcb.take()
    }

    /// This function runs exclusivly on IDLE context
    /// never ending
    pub fn run(&self) -> ! {
        loop {
            intr_on();
            if let Some(proc) = dequeue() {
                verbose!("Found available process {}", proc.pid);
                let mut pcb_inner = proc.get_inner();
                assert!(pcb_inner.status == ProcessStatus::Ready || pcb_inner.status == ProcessStatus::Init);
                if pcb_inner.status != ProcessStatus::Init {
                    pcb_inner.status = ProcessStatus::Running;
                }
                let proc_context = pcb_inner.get_context();
                let mut idle_context = self.get_context();
                let proc_satp = pcb_inner.mem_layout.pagetable.satp(Some(proc.pid));
                let scheuler_satp = self.inner.borrow().sche_mem_layout.as_ref().unwrap().pagetable.satp(None);
                self.inner.borrow_mut().pcb = Some(proc.clone());
                // 1st return form scheduler, pcb_inner is locked for fork_ret();
                // 2nd+ return from scheduler, pcb_inner is locked for to_scheduler().
                unsafe {
                    satp::write(proc_satp);
                    asm!("sfence.vma");
                    __swtch(idle_context, proc_context);
                    satp::write(scheuler_satp);
                    asm!("sfence.vma");
                }
                pcb_inner.check_intergrity();
            } else {
                verbose!("No available process. Processor IDLE.");
                self.stall();
            }
        }
    }

    pub fn stall(&self) {
        intr_on();
        unsafe { asm!("wfi") };
    }

    pub fn to_scheduler(&self, mut proc_inner: MutexGuard<PCBInner>) {
        proc_inner.check_intergrity();
        assert!(proc_inner.status != ProcessStatus::Running, "Current thread must not be running");
        assert!(self.intr_state() == false, "Interrupt must be off to switch to scheduler.");
        assert!(self.get_int_cnt() == 1, "Must only hold one lock when switching to scheduler.");
        let idle_context = self.get_context();
        let proc_context = proc_inner.get_context();
        unsafe {
            __swtch(proc_context, idle_context);
        }
        proc_inner.check_intergrity();
    }
    
    pub fn suspend_switch(&self) {
        let int_ena = get_processor().get_int_ena();
        let int_cnt = get_processor().get_int_cnt();

        let process = self.take_current().expect("Suspend switch need running process to work");
        let mut pcb_inner = process.get_inner();
        pcb_inner.status = ProcessStatus::Ready;
        enqueue(process.clone());

        // pcb_inner was locked for scheduler
        self.to_scheduler(pcb_inner);

        get_processor().set_int_cnt(int_cnt);
        get_processor().set_int_ena(int_ena);
    }

    pub fn exit_switch(&self, exit_code: isize) -> ! {
        let proc = self.take_current().unwrap();
        let mut pcb_inner = proc.get_inner();
        pcb_inner.status = ProcessStatus::Zombie;
        pcb_inner.exit_code = Some(exit_code);

        // let init proc to take all child process
        {
            let mut init_inner = INIT_PROCESS.get_inner();
            for child in &pcb_inner.children {
                child.get_inner().parent = Some(Arc::downgrade(&INIT_PROCESS));
                init_inner.children.insert(child.clone());
            }
        }
        
        pcb_inner.children.clear();
        // HACK: this is ugly as hell someone help me fix this pls
        unsafe {
            // clone, strong count + 1 for this clone will not be dropped but consumed by Arc::into_raw (will be mem::forget()-ed)
            let proc_raw = Arc::into_raw(proc.clone());
            // dec proc.clone() count
            Arc::decrement_strong_count(proc_raw);
            // dec proc count for we are not able to drop it
            Arc::decrement_strong_count(proc_raw);
        }
        self.to_scheduler(pcb_inner);
        unreachable!()
    }

    pub fn activate_mem_layout(&self) {
        info!("Activating mem layout for hart {}", get_hart_id());
        assert!(self.inner.borrow().sche_mem_layout.is_none(), "hart mem layout already initialized.");
        let new_mem_layout = MemLayout::new();
        new_mem_layout.activate();
        self.inner.borrow_mut().sche_mem_layout = Some(new_mem_layout);
        milestone!("Hart {} scheduler memory layout activated.", get_hart_id());
    }

    pub fn get_int_ena(&self) -> bool {
        self.inner.borrow().int_enable_b4_off
    }

    pub fn set_int_ena(&self, ena: bool) {
        self.inner.borrow_mut().int_enable_b4_off = ena;
    }

    pub fn get_int_cnt(&self) -> usize {
        self.inner.borrow().int_off_count
    }

    pub fn set_int_cnt(&self, cnt: usize) {
        self.inner.borrow_mut().int_off_count = cnt;
    }
}

impl ProcessorInner {
    pub fn new() -> Self {
        Self {
            pcb: None,
            int_off_count: 0,
            int_enable_b4_off: false,
            sum_count: 0,
            idle_context: ProcessContext::new(),
            sche_mem_layout: None
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

    pub fn push_sum_on(&mut self) {
        if self.sum_count == 0 {
            unsafe{riscv::register::sstatus::set_sum();}
        }
        self.sum_count += 1;
    }

    pub fn pop_sum_on(&mut self) {
        if self.sum_count == 0 {
            panic!("unmatched pop sum");
        }
        self.sum_count -= 1;
        if self.sum_count == 0 {
            unsafe{riscv::register::sstatus::clear_sum();}
        }
    }
    
    pub fn get_context(&mut self) -> *mut ProcessContext {
        // use identical mapping.
        (&mut self.idle_context) as *mut ProcessContext
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

pub fn get_processor() -> Arc<Processor> {
    return PROCESSOR_MANAGER.get_processor(get_hart_id());
}

// TODO: Change this to RAII style (IntrGuard with new and Drop)
pub fn push_intr_off() {
    let intr_state = sstatus::read().sie();
    intr_off();
    get_processor().register_push_off(intr_state);
}

pub fn pop_intr_off() {
    assert!(!sstatus::read().sie(), "Int was on");
    if get_processor().register_pop_off() {
        intr_on();
    }
}

pub fn intr_off() {
    unsafe {sstatus::clear_sie();}
    assert!(!sstatus::read().sie(), "Cannot clear sie");
}

pub fn intr_on() {
    unsafe {sstatus::set_sie();}
    assert!(sstatus::read().sie(), "Cannot set sie");
}

// TODO: Change this to RAII style (SUMGuard with new and Drop)
pub fn push_sum_on() {
    get_processor().push_sum_on()
}

pub fn pop_sum_on() {
    get_processor().pop_sum_on()
}