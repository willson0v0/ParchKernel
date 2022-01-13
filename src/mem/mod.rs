mod kernel_heap;
mod phys_bitmap;
mod page_allocator;
mod types;
mod pagetable;

pub use kernel_heap::{init_kernel_heap};

pub use types::{
    VirtAddr, 
    PhysAddr
};

pub use page_allocator::{
    alloc_page,
    PageGuard
};