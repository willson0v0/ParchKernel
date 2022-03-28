mod kernel_heap;
mod phys_bitmap;
mod page_allocator;
mod types;
mod pagetable;
mod mem_layout;
mod segment;

pub use phys_bitmap::BitMap;

pub use mem_layout::{
    MemLayout
};

pub use kernel_heap::{init_kernel_heap};

pub use types::{
    VirtAddr, 
    PhysAddr,
    VirtPageNum,
    PhysPageNum,
    VARange,
    PARange,
    VPNRange,
    PPNRange
};

pub use page_allocator::{
    alloc_vm_page,
    alloc_fs_page,
    free_fs_page,
    claim_vm_page,
    claim_fs_page,
    stat_mem,
    PageGuard
};

pub use segment::{
    MMAPType,
    Segment,
    ArcSegment,
    IdenticalMappingSegment,
    ManagedSegment,
    VMASegment,
    TrampolineSegment,
    UTrampolineSegment,
    TrapContextSegment,
    ProcKStackSegment,
    SegmentFlags
};

pub use pagetable::{
    PageTable,
    PageTableEntry,
    PTEFlags
};

use crate::{process::get_processor};

pub fn init() {
    init_kernel_heap();
    verbose!("Kernel heap activated");
    extern "C" {
        fn sbss();
        fn ebss();
    }
    info!("SBSS: {:x}", sbss as usize);
    info!("EBSS: {:x}", ebss as usize);

    // unsafe{ 
    //     let clear_start = sbss as usize as *mut u8;
    //     let length = ebss as usize - sbss as usize;
    //     core::ptr::write_bytes(clear_start, 0, length); 
    // }
    
    milestone!("Memory initialized.");
}

pub fn hart_init() {
    get_processor().activate_mem_layout();
}