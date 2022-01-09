use crate::utils::PhysAddr;

pub const KERNEL_HEAP_SIZE  : usize = 0x100000;

pub const UART0_IRQ: usize = 10;

pub const PLIC_ADDR: PhysAddr = PhysAddr(0x0C000000);

pub const CLINT_ADDR: PhysAddr = PhysAddr(0x02000000);

pub const UART0_ADDR: PhysAddr = PhysAddr(0x10000000);

pub const MAX_CPUS: usize = 16;