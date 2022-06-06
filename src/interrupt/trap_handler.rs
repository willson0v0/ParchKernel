use core::{panic, arch::asm};

use alloc::borrow::ToOwned;
use riscv::register::{scause::{   // s cause register
        self,
        Trap,
        Exception,
        Interrupt,
    }, sepc, sip, sstatus::{self, SPP}, stval, stvec};

// use super::PLIC0;
use crate::{config::{TRAMPOLINE_ADDR, PROC_K_STACK_ADDR, PROC_K_STACK_SIZE, TRAP_CONTEXT_ADDR, PROC_U_STACK_ADDR, PROC_U_STACK_SIZE, U_TRAMPOLINE_ADDR}, interrupt::trap_context::TrapContext, mem::{VirtAddr}, process::{ProcessStatus, SignalNum, def_handler::{usr_sigreturn}, get_hart_id, get_processor, intr_off, intr_on}, syscall::{syscall, syscall_num::SYSCALL_EXEC}, utils::{Mutex, RWLock}};
use crate::device::DEVICE_MANAGER;

/// Set trap entry to kernel trap handling function.
pub fn set_kernel_trap_entry() {
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
    let sepc = sepc::read();

    assert!(sstatus.spp() == SPP::Supervisor, "kerneltrap not from supervisor mode");
    assert!(!sstatus.sie(), "kernel interrupt is enabled");
    
    match scause.cause() {
        // PLIC interrupt
        Trap::Interrupt(Interrupt::SupervisorExternal) => {
            DEVICE_MANAGER.acquire_r().handle_interrupt().unwrap();
            // match PLIC0.plic_claim() {
            //     UART0_IRQ => {
            //         UART0.sync();
            //         PLIC0.plic_complete(UART0_IRQ);
            //     },
            //     0 => {
            //         // do nothing
            //     },
            //     unknown_ext => {
            //         panic!("Unknown external interrupt 0x{:x}", unknown_ext)
            //     }
            // }
        },
        Trap::Interrupt(Interrupt::SupervisorSoft) => {
            // verbose!("Supervisor Soft Interrupt");
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
            // Not doing time like xv6 here, we use CLINT for time.
            // ?: No Timer Vec then?
        },
        Trap::Exception(Exception::InstructionPageFault)    |
        Trap::Exception(Exception::LoadPageFault)           |
        Trap::Exception(Exception::StorePageFault)          => {
            if let Some(proc) = get_processor().current() {
                let proc_inner = unsafe{proc.inner.leak()};
                let lazy_res = proc_inner.mem_layout.do_lazy(VirtAddr::from(stval).into());
                if lazy_res.is_err() {
                    fatal!("Kernel Pagefault, lazy failed with {:?}.", lazy_res.unwrap_err());
                    fatal!("STVAL: {:x}", stval);
                    fatal!("SEPC : {:x}", sepc);
                    panic!("Kernel panic");
                } else {
                    verbose!("kernel lazy done.");
                }
            } else {
                if get_processor().do_lazy(VirtAddr::from(stval).into()).is_err() {
                    fatal!("Kernel Pagefault.");
                    fatal!("STVAL: {:x}", stval);
                    fatal!("SEPC : {:x}", sepc);
                    panic!("Kernel panic");
                }
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
            fatal!("SEPC : {:x}", sepc);
            panic!("Kernel panic");
        }
    }
    unsafe {
        scause::write(scause.bits());
        sepc::write(sepc);
    }
}

#[no_mangle]
pub fn user_trap() -> ! {
    {
        set_kernel_trap_entry();
        let scause = scause::read();
        let stval = stval::read();
        let sstatus = sstatus::read();
        let sepc = sepc::read();
        let trap_context = TrapContext::current_ref();
        trap_context.epc = sepc.into();
    
        assert!(sstatus.spp() == SPP::User, "user_trap not from user mode");
        assert!(!sstatus.sie(), "kernel interrupt is enabled");
        match scause.cause() {
            Trap::Exception(Exception::UserEnvCall) => {
                let syscall_id = trap_context.a7;
                let args = [
                    trap_context.a0,
                    trap_context.a1,
                    trap_context.a2,
                    trap_context.a3,
                    trap_context.a4,
                    trap_context.a5,
                ];
                trap_context.epc += 4;
                intr_on();
                let res = syscall(syscall_id, args);
                if let Ok(ret_val) = res {
                    if syscall_id != SYSCALL_EXEC{
                        trap_context.a0 = ret_val;
                        trap_context.a1 = 0;
                    }
                } else {
                    warning!("Syscall {} failed with {:?}", syscall_id, res);
                    trap_context.a0 = res.unwrap_err().to_ret();
                    trap_context.a1 = usize::MAX;
                }
            },
            Trap::Interrupt(Interrupt::SupervisorTimer) => {
                verbose!("SupervisorTimer");
                get_processor().suspend_switch();
            },
            Trap::Interrupt(Interrupt::SupervisorSoft) => {
                let cleared_sip = sip::read().bits() & !2;
                unsafe {
                    asm! {
                        "csrw sip, {0}",
                        in(reg) cleared_sip
                    };
                }
                assert!(sip::read().bits() & 2 == 0, "Failed to clear ssip");
                verbose!("SupervisorSoft");
                get_processor().suspend_switch();
            },
            // PLIC interrupt
            Trap::Interrupt(Interrupt::SupervisorExternal) => {
                DEVICE_MANAGER.acquire_r().handle_interrupt().unwrap();
                // match PLIC0.plic_claim() {
                //     UART0_IRQ => {
                //         UART0.sync();
                //         PLIC0.plic_complete(UART0_IRQ);
                //     },
                //     0 => {
                //         // do nothing
                //     },
                //     unknown_ext => {
                //         panic!("Unknown external interrupt 0x{:x}", unknown_ext)
                //     }
                // }
            },
            Trap::Exception(Exception::InstructionPageFault)    |
            Trap::Exception(Exception::LoadPageFault)           |
            Trap::Exception(Exception::StorePageFault)          => {
                let proc = get_processor().current().unwrap();
                let mut proc_inner = proc.get_inner();
                if let Err(e) = proc_inner.mem_layout.do_lazy(VirtAddr::from(stval).into()) {
                    fatal!("User Pagefault, do lazy failed with {:?}.", e);
                    fatal!("STVAL: {:x}", stval);
                    fatal!("SEPC : {:x}", sepc);
                    fatal!("User Program dead.");
                    proc_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
                } else {
                    verbose!("User lazy done for {:x}.", stval);
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
                fatal!("User Program dead.");
                let proc = get_processor().current().unwrap();
                let mut proc_inner = proc.get_inner();
                proc_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
            }
        }
    }
    trap_return();
}

#[no_mangle]
pub fn fork_return() -> ! {
    {
        let processor = get_processor();
        let pcb = processor.current().unwrap();
        let mut pcb_inner = unsafe {pcb.inner.from_locked()};   // this was locked in scheduler ( run() ), so it's safe to claim it here
        let trap_context = TrapContext::current_ref();
        if pcb_inner.status == ProcessStatus::Init {
            let elf_file = pcb_inner.elf_file.clone();
            (pcb_inner.entry_point, pcb_inner.data_end) = pcb_inner.mem_layout.map_elf(elf_file).unwrap();
            pcb_inner.status = ProcessStatus::Running;
            *trap_context = TrapContext::new();
            trap_context.epc = pcb_inner.entry_point;
            trap_context.sp = (PROC_U_STACK_ADDR + PROC_U_STACK_SIZE).0;
            debug!("Initialized PCB with entry_point @ {:?}", pcb_inner.entry_point);
        } else {
            info!("First entry to U mode. a0 = {}, a1 = {}, ra {:x}", trap_context.a0, trap_context.a1, trap_context.ra);
        }
    }
    trap_return();
}


#[no_mangle]
pub fn trap_return() -> ! {
    extern "C" {
        fn uservec();
        fn userret();
        fn trampoline();
    }
    {
        intr_off();
        let pcb = get_processor().current().unwrap();
        let mut pcb_inner = pcb.get_inner();
        assert!(pcb_inner.status == ProcessStatus::Running);
        let trap_context = TrapContext::current_ref();
        trap_context.kernel_sp = PROC_K_STACK_ADDR + PROC_K_STACK_SIZE;
        trap_context.user_trap = (user_trap as usize).into();
        trap_context.hart_id = get_hart_id();
        let uservec_addr: VirtAddr = TRAMPOLINE_ADDR + ((uservec as usize) - (trampoline as usize));
    
        // Process pending signal
        // current TrapContext will be archieved
        // new TrapContext will have epc = SignalHandlerVA, ra = __user_restore_from_handler in UTrampoline
        if pcb_inner.pending_signal.len() > 0 {
            let signal = pcb_inner.pending_signal.pop_front().unwrap();
            debug!("Processing signal {:?} for process {:?}", signal, pcb.pid);
            pcb_inner.signal_contexts.push(trap_context.clone());
            
            extern "C" {fn sutrampoline(); }
            let sigreturn_va = U_TRAMPOLINE_ADDR + (usr_sigreturn as usize - sutrampoline as usize);
            trap_context.ra = sigreturn_va.0;
            trap_context.epc = pcb_inner.signal_handler.get(&signal).unwrap().to_owned();
        }
        drop(pcb_inner);
        unsafe {
            stvec::write(uservec_addr.0, stvec::TrapMode::Direct);
            sstatus::set_spie();
            sstatus::set_spp(SPP::User);
            sepc::write(trap_context.epc.0);
        }
    }
    let userret_addr: VirtAddr = TRAMPOLINE_ADDR + ((userret as usize) - (trampoline as usize));
    unsafe {
        let userret_fp: extern "C" fn(VirtAddr) -> ! = core::mem::transmute(userret_addr.0 as *const ());
        userret_fp(TRAP_CONTEXT_ADDR);
    }
}