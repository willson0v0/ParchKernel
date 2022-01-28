use core::{panic};

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

    assert!(sstatus.spp() == SPP::Supervisor, "kerneltrap not from supervisor mode");
    assert!(!sstatus.sie(), "kernel interrupt is enabled");

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
            fatal!("Unexpected scause:");
            match scause.cause() {
                Trap::Exception(exception) => {
                    match exception {
                        Exception::InstructionMisaligned => fatal!("Exception::InstructionMisaligned"),
                        Exception::InstructionFault      => fatal!("Exception::InstructionFault     "),
                        Exception::IllegalInstruction    => fatal!("Exception::IllegalInstruction   "),
                        Exception::Breakpoint            => fatal!("Exception::Breakpoint           "),
                        Exception::LoadFault             => fatal!("Exception::LoadFault            "),
                        Exception::StoreMisaligned       => fatal!("Exception::StoreMisaligned      "),
                        Exception::StoreFault            => fatal!("Exception::StoreFault           "),
                        Exception::UserEnvCall           => fatal!("Exception::UserEnvCall          "),
                        Exception::InstructionPageFault  => fatal!("Exception::InstructionPageFault "),
                        Exception::LoadPageFault         => fatal!("Exception::LoadPageFault        "),
                        Exception::StorePageFault        => fatal!("Exception::StorePageFault       "),
                        Exception::Unknown               => fatal!("Exception::Unknown              "),
                    }
                },
                Trap::Interrupt(interrupt) => {
                    match interrupt {
                        Interrupt::UserSoft             => fatal!("Interrupt::UserSoft             "),
                        Interrupt::SupervisorSoft       => fatal!("Interrupt::SupervisorSoft       "),
                        Interrupt::UserTimer            => fatal!("Interrupt::UserTimer            "),
                        Interrupt::SupervisorTimer      => fatal!("Interrupt::SupervisorTimer      "),
                        Interrupt::UserExternal         => fatal!("Interrupt::UserExternal         "),
                        Interrupt::SupervisorExternal   => fatal!("Interrupt::SupervisorExternal   "),
                        Interrupt::Unknown              => fatal!("Interrupt::Unknown              "),
                    }
                }
            }
            fatal!("STVAL: {:x}", stval);
            fatal!("SEPC : {:x}", sepc::read());
            panic!("Kernel panic");
        }
    }
}