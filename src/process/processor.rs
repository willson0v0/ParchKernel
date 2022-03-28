use core::arch::{asm, global_asm};
use core::cell::{RefCell, Ref};
use core::ops::Deref;


use riscv::register::{
    sstatus, satp,
};
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;
use crate::config::{MAX_CPUS, PROC_K_STACK_ADDR, PROC_K_STACK_SIZE};
use crate::fs::RegularFile;
use crate::interrupt::{fork_return};
use crate::mem::{MemLayout, VirtPageNum, MMAPType};
use crate::process::ProcessControlBlock;
use crate::process::pcb::ProcessStatus;
use crate::utils::{MutexGuard, ErrorNum};

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
    inner: RefCell<ProcessorInner>,
    mem_layout: RefCell<Option<MemLayout>>
}

unsafe impl Sync for Processor{}

pub struct ProcessorInner {
    pub pcb: Option<Arc<ProcessControlBlock>>,
    pub int_off_count: usize,    // depth of push_off nesting
    pub int_enable_b4_off: bool,        // was interrupt enabled before push_off
    pub sum_count: usize,
    pub idle_context: ProcessContext,
    // pub sche_mem_layout: Option<MemLayout>
}

pub struct ProcessorGuard {
    processor: Arc<Processor>
}

impl Deref for ProcessorGuard {
    type Target = Processor;
    fn deref(&self) -> &Self::Target {
        assert!(get_hart_id() == self.processor.hart_id, "CPU access vioaltion");
        assert!(!sstatus::read().sie(), "Interrupt on");
        self.processor.deref()
    }
}

impl Drop for ProcessorGuard {
    fn drop(&mut self) {
        // avoid deref check for it might be back from get_processor.suspend_switch().
        // only drop is explicitly allowed.
        assert!(!sstatus::read().sie(), "Interrupt on");
        let processor = PROCESSOR_MANAGER.get_processor(get_hart_id());
        if processor.register_pop_off() {
            intr_on();
        }
    }
}

impl ProcessorGuard {
    pub fn new() -> Self {
        let intr_state = sstatus::read().sie();
        intr_off();
        let processor = PROCESSOR_MANAGER.get_processor(get_hart_id());
        processor.register_push_off(intr_state);
        Self {processor}
    }

    pub fn ref_cnt(&self) -> usize {
        Arc::strong_count(&self.processor)
    }
}

impl Processor {
    pub fn new(hart_id: usize) -> Self {
        Self {
            hart_id,
            inner: RefCell::new(ProcessorInner::new()),
            mem_layout: RefCell::new(None)
        }
    }
    
    pub fn register_push_off(&self, intr_state: bool) {
        self.inner.borrow_mut().register_push_off(intr_state);
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

    pub fn map_file(&self, file: Arc<dyn RegularFile>) -> VirtPageNum {
        self.mem_layout.borrow_mut().as_mut().unwrap().mmap_file(file.clone(), 0, file.stat().unwrap().file_size, MMAPType::Private).unwrap()
    }

    pub fn unmap_file(&self, start_vpn: VirtPageNum) {
        self.mem_layout.borrow_mut().as_mut().unwrap().remove_segment_by_vpn(start_vpn).unwrap();
    }
    
    pub fn do_lazy(&self, vpn: VirtPageNum) -> Result<(), ErrorNum> {
        self.mem_layout.borrow_mut().as_mut().unwrap().do_lazy(vpn)
    }

    /// This function runs exclusivly on IDLE context
    /// never ending
    pub fn run(&self) -> ! {
        loop {
            intr_on();
            if let Some(proc) = dequeue() {
                let mut pcb_inner = proc.get_inner();
                assert!(pcb_inner.status == ProcessStatus::Ready || pcb_inner.status == ProcessStatus::Init);
                if pcb_inner.status != ProcessStatus::Init {
                    pcb_inner.status = ProcessStatus::Running;
                }
                let proc_context = pcb_inner.get_context();
                let idle_context = self.get_context();
                // pcb_inner.mem_layout.pagetable.print(LogLevel::Verbose);
                let proc_satp = pcb_inner.mem_layout.pagetable.satp(Some(proc.pid));
                let scheuler_satp = self.mem_layout.borrow_mut().as_ref().unwrap().pagetable.satp(None);
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
                // must switched back by to_scheduler, locked by suspend_switch or exit_switch
                pcb_inner.check_intergrity();
            } else {
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
        // one int for one lock, another for ProcessorGuard
        // assert!(self.get_int_cnt() == 2, "Must only hold one lock when switching to scheduler.");
        let idle_context = self.get_context();
        let proc_context = proc_inner.get_context();
        unsafe {
            __swtch(proc_context, idle_context);
        }
        proc_inner.check_intergrity();
    }
    
    pub fn suspend_switch(&self) {
        let processor = get_processor();
        let int_ena = processor.get_int_ena();
        let int_cnt = processor.get_int_cnt();

        let process = self.take_current().expect("Suspend switch need running process to work");
        let mut pcb_inner = process.get_inner();
        pcb_inner.status = ProcessStatus::Ready;
        enqueue(process.clone());

        // pcb_inner was locked for scheduler
        drop(processor);
        self.to_scheduler(pcb_inner);

        let processor = get_processor();
        processor.set_int_cnt(int_cnt);
        processor.set_int_ena(int_ena);
    }

    pub fn exit_switch(&self, exit_code: isize) -> ! {
        // get init first, to avoid deadlock
        // in waitpid, we always get self.inner first, then get childres;
        // so we must use the same lock acquire sequence here,
        // that is, parent first, children last.
        let mut init_inner = INIT_PROCESS.get_inner();
        let proc = self.take_current().unwrap();
        let mut pcb_inner = proc.get_inner();
        pcb_inner.status = ProcessStatus::Zombie;
        pcb_inner.exit_code = Some(exit_code);

        for child in &pcb_inner.children {
            child.get_inner().parent = Some(Arc::downgrade(&INIT_PROCESS));
            init_inner.children.push_back(child.clone());
        }
        
        pcb_inner.children.clear();
        drop(pcb_inner);
        drop(init_inner);
        // deduct proc's refcnt for it will not be dropped.
        // Arc's final drop will not happen here, for parent of this process must held ref to this process, so it's safe to do so.
        unsafe {
            verbose!("count b4 decrement: {}", Arc::strong_count(&proc));
            let arc_ptr = Arc::into_raw(proc);
            Arc::decrement_strong_count(arc_ptr);
            let proc = Arc::from_raw(arc_ptr);
            verbose!("count aft decrement: {}", Arc::strong_count(&proc));
            self.to_scheduler(proc.get_inner());
        }
        unreachable!()
    }

    pub fn activate_mem_layout(&self) {
        info!("Activating mem layout for hart {}", get_hart_id());
        assert!(self.mem_layout.borrow().is_none(), "hart mem layout already initialized.");
        let new_mem_layout = MemLayout::new();
        new_mem_layout.activate();
        *(self.mem_layout.borrow_mut()) = Some(new_mem_layout);
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
            idle_context: ProcessContext::new()
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

pub fn get_processor() -> ProcessorGuard {
    ProcessorGuard::new()
}

// NOTE: can use ProcessorGuard as RAII version of push_intr_on / off.
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