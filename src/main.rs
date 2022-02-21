#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(exclusive_range_pattern)]
#![feature(panic_info_message)]
#![feature(step_trait)]
#![allow(dead_code)]

#[macro_use]
mod utils;
mod mem;
mod config;
mod interrupt;
mod version;
mod fs;
mod process;

#[macro_use]
extern crate alloc;
extern crate lazy_static;
extern crate static_assertions;

use core::{arch::{global_asm, asm}};

global_asm!(include_str!("crt_setup.asm"));
global_asm!(include_str!("interrupt/kernel_trap.asm"));
global_asm!(include_str!("interrupt/trampoline.asm"));


use riscv::register::{medeleg, mepc, mhartid, mideleg, mie, mscratch, mstatus, mtvec, pmpaddr0, pmpcfg0, satp, sie, sstatus, stvec};

static mut MSCRATCH_ARR: [[usize; 6]; config::MAX_CPUS] = [[0; 6]; config::MAX_CPUS];

#[no_mangle]
extern "C" fn genesis_m() -> ! {
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
        let hart_id = mhartid::read();
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
            "mv tp, {0}",
            "mret",
            in(reg) hart_id
        };
    }
    
    unreachable!()
}

#[no_mangle]
extern "C" fn genesis_s() {
    if interrupt::get_hart_id() == 0 {
        // common init code (mm/fs)
        unsafe {
            extern "C" {
                fn kernel_vec();
            }
            sstatus::set_sie();
            stvec::write(kernel_vec as usize, stvec::TrapMode::Direct)
        }
        mem::init();
        println!("\r\n\n\n\nParch OS\n");
        println!("Ver\t: {}", version::VERSION);
    } else {
        // hart specific init code
    }
    
    loop{
        print!("a");
    }
}