#![no_std]
#![no_main]
#![feature(llvm_asm)]
#![feature(global_asm)]
#![feature(alloc_error_handler)]
#![feature(exclusive_range_pattern)]

mod utils;
mod mem;
mod config;
mod interrupt;

extern crate alloc;

global_asm!(include_str!("crt_setup.asm"));

#[no_mangle]
extern "C" fn genesis() {
    mem::init_kernel_heap();
    loop{}
}