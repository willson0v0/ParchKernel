use crate::{utils::{Mutex, SpinMutex}, config::PAGE_SIZE};
use alloc::sync::Arc;
use lazy_static::*;
use super::{PhysAddr, phys_bitmap::BitMap, types::PhysPageNum};

extern "C" {
	fn ekernel();
	fn BASE_ADDRESS();
	fn PHYS_END_ADDRESS ();
	fn SUPERBLOCK_ADDRESS();
	fn PAGE_BITMAP_FS_ADDRESS();
	fn PAGE_BITMAP_MM_ADDRESS();
	fn INODE_BITMAP_ADDRESS();
	fn INODE_LIST_ADDRESS();
}

lazy_static!{
	static ref PAGE_ALLOCATOR: SpinMutex<BitMapPageAllocator> = {
		SpinMutex::new(
			"PageAllocator", 
			BitMapPageAllocator::new(
				(ekernel as usize).into(),
				(INODE_BITMAP_ADDRESS as usize) - (ekernel as usize)
			)
		)
	};
}

trait PageAllocator {
	fn new(begin: PhysAddr, length: usize) -> Self;
	fn alloc(&mut self, is_exec: bool) -> Option<PhysPageNum>;
	fn free(&mut self, to_free: PhysPageNum, is_exec: bool);
	fn claim(&mut self, to_claim: PhysPageNum, is_exec: bool);
}

pub type PageGuard = Arc<PageGuardInner>;

pub struct PageGuardInner {
	pub ppn: PhysPageNum,
	pub is_exec: bool
}

impl PageGuardInner {
	pub fn new(ppn: PhysPageNum, is_exec: bool) -> Self {
		Self {ppn, is_exec}
	}
}

impl Drop for PageGuardInner {
	fn drop(&mut self) {
		PAGE_ALLOCATOR.acquire().free(self.ppn, self.is_exec);
	}
}

/// bitmap_fs is for all allocated page, either for exec or file
/// bitmap_mm is for exec memory, and overlaps with bitmap_fs
pub struct BitMapPageAllocator {
	bitmap_mm: BitMap,
	bitmap_fs: BitMap
}

impl BitMapPageAllocator {
	fn mark_unavailable(&mut self, ppn: PhysPageNum, is_exec: bool) {
		let index = ppn - PhysPageNum::from(BASE_ADDRESS as usize);
		assert!(!self.bitmap_fs.get(index), "Already allocated");
		if is_exec {
			self.bitmap_mm.set(index);
		}
		self.bitmap_fs.set(index);
	}

	fn mark_available(&mut self, ppn: PhysPageNum, is_exec: bool) {
		let index = ppn - PhysPageNum::from(BASE_ADDRESS as usize);
		if is_exec {
			self.bitmap_mm.clear(index);
		}
		self.bitmap_fs.clear(index);
	}
}

impl PageAllocator for BitMapPageAllocator {
    fn new(begin: PhysAddr, length: usize) -> Self {
        let mut res = Self {
			bitmap_mm: BitMap::new((PAGE_BITMAP_MM_ADDRESS as usize).into(), PAGE_BITMAP_FS_ADDRESS as usize - PAGE_BITMAP_MM_ADDRESS as usize),
			bitmap_fs: BitMap::new((PAGE_BITMAP_FS_ADDRESS as usize).into(), SUPERBLOCK_ADDRESS as usize - PAGE_BITMAP_FS_ADDRESS as usize)
		};

		// mark unavailable
		let mut i: PhysPageNum = (BASE_ADDRESS as usize).into();
		while i <=  PhysAddr::from(PHYS_END_ADDRESS as usize).to_ppn_ceil() {
			if i < begin.to_ppn_ceil() || i >= (begin + length).into() {
				res.mark_unavailable(i, true)
			}
			i += 1;
		}

		res
    }

    fn alloc(&mut self, is_exec: bool) -> Option<PhysPageNum> {
		// TODO: Fill with junk like xv6?
		// first_empty in fs, for fs occupy pages too
		self.bitmap_fs.first_empty().and_then(
			|block_id: usize| -> Option<PhysPageNum> {
				if is_exec {
					assert!(!self.bitmap_mm.get(block_id), "Already allocated as exec");
				}
				assert!(!self.bitmap_fs.get(block_id), "Already allocated as fs");
				let ppn = PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize)) + block_id;
				self.mark_unavailable(ppn, is_exec);
				Some(ppn)
			}
		)
    }

    fn free(&mut self, to_free: PhysPageNum, is_exec: bool) {
		// TODO: Fill with junk like xv6?
		let block_id = to_free - PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize));
		if is_exec {
			assert!(self.bitmap_mm.get(block_id), "Freeing non-exec page");
		} else {
			assert!(!self.bitmap_mm.get(block_id), "Freeing exec page");
		}
		assert!(self.bitmap_fs.get(block_id), "Freeing free page");
        self.mark_available(to_free, is_exec);
    }

    fn claim(&mut self, to_claim: PhysPageNum, is_exec: bool) {
        self.mark_unavailable(to_claim, is_exec);
    }
}

pub fn alloc_page(is_exec: bool) -> PageGuard {
	PageGuard::new(PageGuardInner::new(PAGE_ALLOCATOR.acquire().alloc(is_exec).unwrap(), is_exec))
}

pub fn claim_page(to_claim: PhysPageNum, is_exec: bool) -> PageGuard {
	PAGE_ALLOCATOR.acquire().claim(to_claim, is_exec);
	PageGuard::new(PageGuardInner::new(to_claim, is_exec))
}