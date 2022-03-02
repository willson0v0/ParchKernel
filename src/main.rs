#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(exclusive_range_pattern)]
#![feature(panic_info_message)]
#![feature(step_trait)]
#![feature(is_some_with)]
#![allow(dead_code)]
#![deny(unused_must_use)]

// lock sequence
// 
// CPU
// PCBInner
// FileInner
// ParchFSInner
// INode

#[macro_use]
mod utils;
mod mem;
mod config;
mod interrupt;
mod version;
mod fs;
mod process;
mod syscall;

#[macro_use]
extern crate alloc;
extern crate lazy_static;
extern crate static_assertions;
extern crate elf_rs;

// #[macro_use]
// extern crate alloc_no_stdlib;

use core::{arch::{global_asm, asm}, sync::atomic::{AtomicBool, Ordering}};

global_asm!(include_str!("crt_setup.asm"));
global_asm!(include_str!("interrupt/kernel_trap.asm"));
global_asm!(include_str!("interrupt/trampoline.asm"));
global_asm!(include_str!("interrupt/u_trampoline.asm"));


use riscv::register::{medeleg, mepc, mhartid, mideleg, mie, mscratch, mstatus, mtvec, pmpaddr0, pmpcfg0, satp, sie};

use crate::process::get_hart_id;

#[no_mangle]
#[link_section = ".bss"]
static mut MSCRATCH_ARR: [[usize; 6]; config::MAX_CPUS] = [[0; 6]; config::MAX_CPUS];
#[no_mangle]
#[link_section = ".bss"]
static mut HART_REGISTER: [bool; config::MAX_CPUS] = [false; config::MAX_CPUS];
#[no_mangle]
#[link_section = ".bss"]
static BOOT_FIN: AtomicBool = AtomicBool::new(false);

#[no_mangle]
extern "C" fn genesis_m(hart_id: usize) -> ! {
    // set mstatus previous privilege
    extern "C" {
        fn genesis_s();
        fn timervec();
    }
    // every hart will go through this and set their tps
    unsafe {
        // set previous priviledge mode
        mstatus::set_mpp(mstatus::MPP::Supervisor);
        // set mepc
        mepc::write(genesis_s as usize);
        // enable fpu
        mstatus::set_fs(mstatus::FS::Initial);
        // diable paging
        satp::set(satp::Mode::Bare, 0, 0);
        // set deleg to s mode
        medeleg::set_breakpoint();
        medeleg::set_illegal_instruction();
        medeleg::set_instruction_fault();
        medeleg::set_instruction_misaligned();
        medeleg::set_instruction_page_fault();
        medeleg::set_load_fault();
        medeleg::set_load_misaligned();
        medeleg::set_load_page_fault();
        medeleg::set_machine_env_call();
        medeleg::set_store_fault();
        medeleg::set_store_misaligned();
        medeleg::set_store_page_fault();
        medeleg::set_supervisor_env_call();
        medeleg::set_user_env_call();
        mideleg::set_sext();
        mideleg::set_ssoft();
        mideleg::set_stimer();
        mideleg::set_uext();
        mideleg::set_usoft();
        mideleg::set_utimer();
        // set s intr
        sie::set_sext();
        sie::set_ssoft();
        sie::set_stimer();  // seems deprecated, can only go with mtimecmp -> mtime -> sip -> s-mode soft int now
        sie::set_uext();
        sie::set_usoft();
        sie::set_utimer();
        // set phys addr protection
        pmpaddr0::write(0x3fffffffffffffusize);
        pmpcfg0::write(0xfusize);
        HART_REGISTER[hart_id] = true;
        // set timer interrupt and set up mscratch
        // mscratch for the cpu will store registers used in timervec
        // scratch[0,1,2] : register save area.
        // scratch[4] : address of CLINT's MTIMECMP register.
        // scratch[5] : desired interval between interrupts.
        interrupt::CLINT.set_mtimecmp(hart_id, interrupt::CLINT.get_time() + (config::CLOCK_FREQ / config::TIMER_FRAC) as usize);
        MSCRATCH_ARR[hart_id][4] = (config::CLINT_ADDR + 0x4000 + 8 * hart_id).0;
        MSCRATCH_ARR[hart_id][5] = config::CLOCK_FREQ / config::TIMER_FRAC;
        mscratch::write(MSCRATCH_ARR[hart_id].as_ptr() as usize);
        mtvec::write(timervec as usize, mtvec::TrapMode::Direct);
        // only enableling timer interrupt so should be fine
        mie::set_mtimer();
        mstatus::set_mie();
        // set thread pointer and return
        asm! {
            "mret"
        };
    }
    
    unreachable!()
}


#[no_mangle]
extern "C" fn genesis_s() -> ! {
    process::intr_off();
    interrupt::set_kernel_trap_entry();
    if get_hart_id() == 0 {
        // common init code (mm/fs)
        mem::init();
        mem::hart_init();

        interrupt::init();
        interrupt::init_hart();
        
        println!("\r\n\n\n\nParch OS\n");
        println!("Ver\t: {}", version::VERSION);

        fs::init();

        process::init();

        milestone!("Hart 0 boot sequence done.");
        {BOOT_FIN.store(true, Ordering::Release);}
    } else {
        interrupt::set_kernel_trap_entry();
        while !BOOT_FIN.load(Ordering::Acquire) {}
        mem::hart_init();
        interrupt::init_hart();
    }
    
    process::intr_on();
    process::hart_init();
    
    unreachable!();
}