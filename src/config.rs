use crate::mem::{PhysAddr, VirtAddr};

pub const KERNEL_HEAP_SIZE  : usize = 0x100_0000;   // 16MiB
pub const PROC_K_STACK_SIZE : usize = 0x10_0000;    // 1MiB
pub const PROC_U_STACK_SIZE : usize = 0x10_0000;    // 1MiB
pub const PAGE_OFFSET		: usize = 12;
pub const PAGE_SIZE			: usize = 1 << PAGE_OFFSET;
pub const UART0_IRQ			: u32 = 10;
pub const CLINT_ADDR		: PhysAddr = PhysAddr(0x02000000);
pub const PLIC_ADDR			: PhysAddr = PhysAddr(0x0C000000);
pub const UART0_ADDR		: PhysAddr = PhysAddr(0x10000000);
pub const PHYS_END_ADDR		: PhysAddr = PhysAddr(0x1_0000_0000);
pub const PHYS_START_ADDR	: PhysAddr = PhysAddr(0x8000_0000);
pub const MMIO_RANGES       : &[(usize, usize)] = &[
    (0x0200_0000, 0x0201_0000),     /* CLint     */
    (0x0C00_0000, 0x1000_0000),     /* PLIC      */
    (0x1000_0000, 0x1000_1000),     /* UART      */ 
];

pub const TRAMPOLINE_ADDR   : VirtAddr = VirtAddr(usize::MAX - PAGE_SIZE + 1);
pub const U_TRAMPOLINE_ADDR : VirtAddr = VirtAddr(TRAMPOLINE_ADDR.0 - PAGE_SIZE);
pub const TRAP_CONTEXT_ADDR : VirtAddr = VirtAddr(U_TRAMPOLINE_ADDR.0 - PAGE_SIZE);
pub const PROC_K_STACK_ADDR : VirtAddr = VirtAddr(TRAP_CONTEXT_ADDR.0 - PAGE_SIZE - PROC_K_STACK_SIZE);
pub const PROC_U_STACK_ADDR : VirtAddr = VirtAddr(PROC_K_STACK_ADDR.0 - PAGE_SIZE - PROC_U_STACK_SIZE);


pub const MAX_CPUS			: usize = 16;	
pub const CLOCK_FREQ		: usize = 0x00989680;   // from dtb
pub const CYCLE_PER_TICK    : usize = 0x100;
pub const TIMER_FRAC		: usize = 1;	// trigger every 100ms

pub const INIT_PROCESS_PATH      : &str = "/init_proc";

pub const MAX_FD            : usize = 4096;
pub const MAX_SYSCALL       : usize = 64;

pub const MAX_LINK_RECURSE  : usize = 32;

pub const UUID_LENGTH       : usize = 16;  // 16 bytes
pub const PIPE_BUFFER_MAX   : usize = 4096;