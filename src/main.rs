#![no_std]
#![no_main]
#![feature(asm)]
#![feature(global_asm)]
#![feature(alloc_error_handler)]
#![feature(exclusive_range_pattern)]

mod utils;
mod mem;
mod config;
mod interrupt;

extern crate alloc;
extern crate lazy_static;

global_asm!(include_str!("crt_setup.asm"));

use riscv::register::{
    mstatus,
    mepc,
    satp,
    medeleg,
    mideleg,
    sie,
    pmpaddr0,
    pmpcfg0,
    mhartid
};

#[no_mangle]
extern "C" fn genesis_m() {
    // set mstatus previous privilege
    extern "C" {
        fn genesis_s();
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
        sie::set_stimer();
        sie::set_uext();
        sie::set_usoft();
        sie::set_utimer();
        // set phys addr protection
        pmpaddr0::write(0x3fffffffffffffusize);
        pmpcfg0::write(0xfusize);
        let hart_id = mhartid::read();
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
    mem::init_kernel_heap();
    print!("Hello world!");
    loop{}
}