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


pub const SUPERBLOCK_ADDR   : PhysAddr = PhysAddr(0xFFFF_F000);     
pub const PAGE_BITMAP_ADDR  : PhysAddr = PhysAddr(0xFFFE_F000);     
pub const INODE_BITMAP_ADDR : PhysAddr = PhysAddr(0xFFDE_F000);     

pub const SUPERBLOCK_SIZE   : usize = PHYS_END_ADDR.0 - SUPERBLOCK_ADDR.0;          // 1 page
pub const PAGE_BITMAP_SIZE  : usize = SUPERBLOCK_ADDR.0 - PAGE_BITMAP_ADDR.0;       // 16 pages
pub const INODE_BITMAP_SIZE : usize = PAGE_BITMAP_ADDR.0 - INODE_BITMAP_ADDR.0;     // 512 pages

pub const MAX_CPUS			: usize = 16;	
pub const CLOCK_FREQ		: usize = 12500000;
pub const TIMER_FRAC		: usize = 10;	// trigger every 1/10 second