use core::{panic, arch::asm};

use riscv::register::{scause::{   // s cause register
        self,
        Trap,
        Exception,
        Interrupt,
    }, sepc, sip, sstatus::{self, SPP}, stval, stvec};

use super::PLIC0;
use crate::{config::{UART0_IRQ, TRAMPOLINE_ADDR, PROC_K_STACK_ADDR, PROC_K_STACK_SIZE, TRAP_CONTEXT_ADDR, PROC_U_STACK_ADDR, PROC_U_STACK_SIZE}, process::{get_processor, ProcessStatus, intr_off, get_hart_id}, mem::VirtAddr, interrupt::trap_context::TrapContext};
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
            assert!(sip::read().bits() & 2 == 0, "Failed to clear ssip");
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

#[no_mangle]
pub fn user_trap() -> ! {
    todo!()
}

#[no_mangle]
pub fn trap_return() -> ! {
    let pcb = get_processor().current().unwrap();
    let mut pcb_inner = pcb.get_inner();
    let mut trap_context = TrapContext::get_ref();
    if pcb_inner.status == ProcessStatus::Initialized {
        pcb_inner.entry_point = pcb_inner.mem_layout.map_elf(pcb.elf_file.clone()).unwrap();
        pcb_inner.status = ProcessStatus::Running;
        trap_context.epc = pcb_inner.entry_point;
        trap_context.sp = (PROC_U_STACK_ADDR + PROC_U_STACK_SIZE).0;
    }
    trap_context.kernel_sp = PROC_K_STACK_ADDR + PROC_K_STACK_SIZE;
    trap_context.user_trap = (user_trap as usize).into();
    trap_context.hart_id = get_hart_id();
    extern "C" {
        fn uservec();
        fn userret();
        fn trampoline();
    }
    let uservec_addr: VirtAddr = TRAMPOLINE_ADDR + ((uservec as usize) - (trampoline as usize));
    let userret_addr: VirtAddr = TRAMPOLINE_ADDR + ((userret as usize) - (trampoline as usize));
    intr_off();
    unsafe {
        let userret_fp: extern "C" fn(VirtAddr) -> ! = core::mem::transmute(userret_addr.0 as *const ());
        stvec::write(uservec_addr.0, stvec::TrapMode::Direct);
        sstatus::set_spie();
        sstatus::set_spp(SPP::User);
        sepc::write(trap_context.epc.0);
        userret_fp(TRAP_CONTEXT_ADDR);
    }
    unreachable!("Trap return unreachable")
}