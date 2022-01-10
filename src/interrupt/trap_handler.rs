use core::panic;

use riscv::register::{scause::{   // s cause register
        self,
        Trap,
        Exception,
        Interrupt,
    }, sepc, sie, sip, sstatus::{self, SPP}, stval, stvec};

use super::PLIC0;
use crate::config::UART0_IRQ;
use crate::utils::UART0;

/// Set trap entry to kernel trap handling function.
fn set_kernel_trap_entry() {
    unsafe {
        extern "C" {
            fn kernel_vec();
        }
        stvec::write(kernel_vec as usize, stvec::TrapMode::Direct);
    }
}

#[no_mangle]
pub fn kernel_trap() {
    let scause = scause::read();
    let stval = stval::read();
    let sstatus = sstatus::read();

    if sstatus.spp() != SPP::Supervisor {
        panic!("kerneltrap not from supervisor mode,")
    }
    if sstatus.sie() {
        panic!("kernel interrupt is enabled.")
    }

    match scause.cause() {
        // PLIC interrupt
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            match PLIC0.plic_claim() {
                UART0_IRQ => {
                    UART0.sync();
                },
                _ => {
                    panic!("Unknown external interrupt")
                }
            }
        },
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
            debug!("Supervisor Soft Interrupt");
            // riscv::register::sip
            // for some reason sip was not provided with write interface...
            let cleared_sip = sip::read().bits() & !2;
            unsafe {
                asm! {
                    "csrw sip, {0}",
                    in(reg) cleared_sip
                };
            }
        },
        _ => {
            panic!("Unexpected scause")
        }
    }
}