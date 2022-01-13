use crate::{config::{INODE_BITMAP_ADDR, PAGE_BITMAP_ADDR, PAGE_BITMAP_SIZE, PHYS_END_ADDR, PHYS_START_ADDR}, utils::{Mutex, SpinMutex}};
use alloc::sync::Arc;
use lazy_static::*;
use super::{PhysAddr, phys_bitmap::BitMap, types::PhysPageNum};

lazy_static!{
	static ref PAGE_ALLOCATOR: SpinMutex<BitMapPageAllocator> = {
		extern "C" {
			fn ekernel();
		}
		SpinMutex::new(
			"PageAllocator", 
			BitMapPageAllocator::new(
				(ekernel as usize).into(),
				(INODE_BITMAP_ADDR - (ekernel as usize)).0
			)
		)
	};
}

trait PageAllocator {
	fn new(begin: PhysAddr, length: usize) -> Self;
	fn alloc(&mut self) -> Option<PhysPageNum>;
	fn free(&mut self, to_free: PhysPageNum);
	fn claim(&mut self, to_claim: PhysPageNum);
}

pub type PageGuard = Arc<PageGuardInner>;

pub struct PageGuardInner {
	pub ppn: PhysPageNum
}

impl PageGuardInner {
	pub fn new(ppn: PhysPageNum) -> Self {
		Self {ppn}
	}
}

impl Drop for PageGuardInner {
	fn drop(&mut self) {
		PAGE_ALLOCATOR.acquire().free(self.ppn);
	}
}

pub struct BitMapPageAllocator {
	bitmap: BitMap
}

impl BitMapPageAllocator {
	fn mark_unavailable(&mut self, ppn: PhysPageNum) {
		let index = ppn - PhysPageNum::from(PHYS_START_ADDR);
		assert!(!self.bitmap.get(index), "Marking used physical page");
		self.bitmap.set(index);
	}

	fn mark_available(&mut self, ppn: PhysPageNum) {
		let index = ppn - PhysPageNum::from(PHYS_START_ADDR);
		assert!(self.bitmap.get(index), "Freeing free page");
		self.bitmap.clear(index);
	}
}

impl PageAllocator for BitMapPageAllocator {
    fn new(begin: PhysAddr, length: usize) -> Self {
        let mut res = Self {
			bitmap: BitMap::new(PAGE_BITMAP_ADDR, PAGE_BITMAP_SIZE)
		};

		// mark unavailable
		let mut i: PhysPageNum = PHYS_START_ADDR.into();
		while i <=  PHYS_END_ADDR.to_ppn_ceil() {
			if i < begin.to_ppn_ceil() || i >= (begin + length).into() {
				res.mark_unavailable(i)
			}
			i += 1;
		}

		res
    }

    fn alloc(&mut self) -> Option<PhysPageNum> {
		// TODO: Fill with junk like xv6?
		self.bitmap.first_empty().and_then(
			|ppn: usize| -> Option<PhysPageNum> {
				self.mark_unavailable(ppn.into());
				Some(ppn.into())
			}
		)
    }

    fn free(&mut self, to_free: PhysPageNum) {
		// TODO: Fill with junk like xv6?
        self.mark_available(to_free);
    }

    fn claim(&mut self, to_claim: PhysPageNum) {
        self.mark_unavailable(to_claim);
    }
}

pub fn alloc_page() -> PageGuard {
	PageGuard::new(PageGuardInner::new(PAGE_ALLOCATOR.acquire().alloc().unwrap()))
}

pub fn claim_page(to_claim: PhysPageNum) -> PageGuard {
	PAGE_ALLOCATOR.acquire().claim(to_claim);
	PageGuard::new(PageGuardInner::new(to_claim))
}