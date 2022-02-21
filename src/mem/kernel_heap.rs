//! Kernem dynamic memory allocator for oshit kernel.

use buddy_system_allocator::LockedHeap;
use crate::config::KERNEL_HEAP_SIZE;



/// The global allocator, enables us to use extern alloc crate.
#[global_allocator]
static KERNEL_HEAP_ALLOCATOR: LockedHeap<64> = LockedHeap::empty();

/// The empty space to use as kernel heap.
static mut HEAP_SPACE: [u8; KERNEL_HEAP_SIZE] = [0; KERNEL_HEAP_SIZE];

/// Initialized the kernel heap  
/// *Don't call this multiple times!*
pub fn init_kernel_heap() {
    unsafe {
        KERNEL_HEAP_ALLOCATOR.lock().init(HEAP_SPACE.as_ptr() as usize, KERNEL_HEAP_SIZE);
    }
}

/// Alloc error handler
/// Panic on allocation error.
#[alloc_error_handler]
pub fn on_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Kernel heap allocation error on allocating layout {:?}. OOM?", layout);
}