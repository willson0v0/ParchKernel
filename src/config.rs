use crate::mem::PhysAddr;

pub const KERNEL_HEAP_SIZE  : usize = 0x100000;
pub const PAGE_OFFSET		: usize = 12;
pub const PAGE_SIZE			: usize = 1 << PAGE_OFFSET;
pub const UART0_IRQ			: u32 = 10;
pub const PLIC_ADDR			: PhysAddr = PhysAddr(0x0C000000);
pub const CLINT_ADDR		: PhysAddr = PhysAddr(0x02000000);
pub const UART0_ADDR		: PhysAddr = PhysAddr(0x10000000);
pub const PHYS_END_ADDR		: PhysAddr = PhysAddr(0x1_0000_0000);
pub const PHYS_START_ADDR	: PhysAddr = PhysAddr(0x8000_0000);

pub const MAX_CPUS			: usize = 16;	
pub const CLOCK_FREQ		: usize = 12500000;
pub const TIMER_FRAC		: usize = 10;	// trigger every 1/10 second