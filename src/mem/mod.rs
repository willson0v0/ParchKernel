mod kernel_heap;
mod phys_bitmap;
mod page_allocator;
mod types;
mod pagetable;
mod mem_layout;
mod segment;

pub use mem_layout::{
    MemLayout,
    SCHEDULER_MEM_LAYOUT
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
    alloc_page,
    PageGuard
};

pub use segment::{
    Segment,
    IdenticalMappingSegment,
    ManagedSegment,
    VMASegment,
    TrampolineSegment,
    UTrampolineSegment,
    TrapContextSegment
};

pub use pagetable::{
    PageTable,
    PageTableEntry,
    PTEFlags
};

use crate::utils::Mutex;

pub fn init() {
    init_kernel_heap();
    verbose!("Kernel heap activated");
    extern "C" {
        fn sbss();
        fn ebss();
    }
    info!("SBSS: {:x}", sbss as usize);
    info!("EBSS: {:x}", ebss as usize);
    // for i in PARange::new((sbss as usize).into(), (ebss as usize).into()) {
    //     unsafe{ i.write_volatile(&0u8); }
    // }
    SCHEDULER_MEM_LAYOUT.acquire().activate();
}