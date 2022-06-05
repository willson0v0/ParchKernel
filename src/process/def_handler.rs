use core::arch::asm;
use crate::{syscall::syscall_num::{SYSCALL_EXIT, SYSCALL_SIGRETURN}, config::U_TRAMPOLINE_ADDR};

#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
pub fn def_terminate_self(_: isize) {
    unsafe {
        asm!(
            "ecall",
            in("a7") SYSCALL_EXIT
        )
    }
}

#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
pub fn def_ignore(_: isize) {
	// do nothing
}

#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
pub fn def_dump_core(_: isize) {
	// do nothing. for now.
    // TODO: Add proper core dump function.
    
    unsafe {
        asm!(
            "ecall",
            in("a7") SYSCALL_EXIT
        )
    }
}

#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
pub fn usr_sigreturn() {
    unsafe {
        asm!(
            "ecall",
            in("a7") SYSCALL_SIGRETURN
        )
    }
}


#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
pub static PROC_STOPPED: bool = true;

/// to prevent r page fault, for U_TRAMPOLINE_ADDR is in kernel
/// so copy this to u_trampoline
#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
static U_TRAMPOLINE: usize = U_TRAMPOLINE_ADDR.0;

#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
pub fn def_stop(_: isize) {
    extern "C" {
        fn sutrampoline();
    }
    unsafe {
        let proc_stopped: *mut bool = ((&PROC_STOPPED as *const bool) as usize - sutrampoline as usize + U_TRAMPOLINE) as *mut bool;
        *proc_stopped = true;
        
        while *proc_stopped {}
    }
}

#[no_mangle]
#[link_section = ".text.u_trampoline_rust"]
pub fn def_cont(_: isize) {
    extern "C" {
        fn sutrampoline();
    }
    unsafe {
        let proc_stopped: *mut bool = ((&PROC_STOPPED as *const bool) as usize - sutrampoline as usize + U_TRAMPOLINE) as *mut bool;
        *proc_stopped = false;
    }
}