mod trap_handler;
mod plic;
mod clint;
pub mod int_callback;
pub mod trap_context;

// pub use plic::PLIC0;

pub use clint::CLINT;

pub use trap_handler::{trap_return, set_kernel_trap_entry, fork_return};

use crate::config::UART0_IRQ;

// pub fn init() {
//     PLIC0.enable_irqs_priority(vec![UART0_IRQ]);
//     milestone!("PLIC0 irq oriority set.");
// }

// pub fn init_hart() {
//     PLIC0.enable_irqs_hart(vec![UART0_IRQ]);
//     milestone!("PLIC0 irq enabled.");
// }