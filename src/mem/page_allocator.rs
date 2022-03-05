use crate::{utils::{Mutex, SpinMutex}};
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
		verbose!("Initializing page allocator.");
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
	fn stat(&self) -> (usize, usize);
}

pub type PageGuard = Arc<PageGuardInner>;

pub struct PageGuardInner {
	pub ppn: PhysPageNum,
	pub is_exec: bool,
	pub do_free: bool
}

impl PageGuardInner {
	pub fn new(ppn: PhysPageNum, is_exec: bool, do_free: bool) -> Self {
		Self {ppn, is_exec, do_free}
	}
}

impl Drop for PageGuardInner {
	fn drop(&mut self) {
		if self.do_free {
			PAGE_ALLOCATOR.acquire().free(self.ppn, self.is_exec);
		}
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
		let index = ppn - PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize));
		if is_exec {
			self.bitmap_mm.set(index);
		}
		self.bitmap_fs.set(index);
	}

	fn mark_available(&mut self, ppn: PhysPageNum, is_exec: bool) {
		let index = ppn - PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize));
		if is_exec {
			self.bitmap_mm.clear(index);
		}
		self.bitmap_fs.clear(index);
	}
}

impl PageAllocator for BitMapPageAllocator {
    fn new(begin: PhysAddr, length: usize) -> Self {
		verbose!("Initializeing BitMapPageAllocator");
        let mut res = Self {
			bitmap_mm: BitMap::new((PAGE_BITMAP_MM_ADDRESS as usize).into(), (PAGE_BITMAP_FS_ADDRESS as usize - PAGE_BITMAP_MM_ADDRESS as usize) * 8),
			bitmap_fs: BitMap::new((PAGE_BITMAP_FS_ADDRESS as usize).into(), (SUPERBLOCK_ADDRESS as usize - PAGE_BITMAP_FS_ADDRESS as usize) * 8)
		};

		// mark unavailable
		let mut i: PhysPageNum = (BASE_ADDRESS as usize).into();
		while i <=  PhysAddr::from(PHYS_END_ADDRESS as usize).to_ppn_ceil() {
			if i < begin.to_ppn_ceil() || i >= (begin + length).into() {
				res.mark_unavailable(i, true)
			}
			i += 1;
		}

		debug!("BitMapPageAllocator initialized.");
		res
    }

    fn alloc(&mut self, is_exec: bool) -> Option<PhysPageNum> {
		// first_empty in fs, for fs occupy pages too
		self.bitmap_fs.first_empty().and_then(
			|block_id: usize| -> Option<PhysPageNum> {
				if is_exec {
					assert!(!self.bitmap_mm.get(block_id), "Already allocated as exec");
				}
				assert!(!self.bitmap_fs.get(block_id), "Already allocated as fs");
				let ppn = PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize)) + block_id;
				// verbose!("Alloced: {:?}", ppn);
				self.mark_unavailable(ppn, is_exec);
				if cfg!(debug_assertions) {
					unsafe{ppn.clear_content();}
				}
				Some(ppn)
			}
		)
    }

    fn free(&mut self, to_free: PhysPageNum, is_exec: bool) {
		let block_id = to_free - PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize));
		if is_exec {
			assert!(self.bitmap_mm.get(block_id), "Freeing non-exec page");
		} else {
			assert!(!self.bitmap_mm.get(block_id), "Freeing exec page");
		}
		assert!(self.bitmap_fs.get(block_id), "Freeing free page");
		if cfg!(debug_assertions) {
			unsafe{to_free.clear_content();}
		}
        self.mark_available(to_free, is_exec);
    }

    fn claim(&mut self, to_claim: PhysPageNum, is_exec: bool) {
        self.mark_unavailable(to_claim, is_exec);
    }

	fn stat(&self) -> (usize, usize) {
		(self.bitmap_fs.count(), self.bitmap_mm.count())
	}
}

pub fn alloc_vm_page() -> PageGuard {
	let ppn = PAGE_ALLOCATOR.acquire().alloc(true).unwrap();
	if cfg!(debug_assertions) {
		unsafe{ppn.clear_content();}
	}
	PageGuard::new(PageGuardInner::new(ppn, true, true))
}

/// fs pages persist across boots, so RAII won't work for them, must explicit free
pub fn alloc_fs_page() -> PhysPageNum {
	let ppn = PAGE_ALLOCATOR.acquire().alloc(false).unwrap();
	if cfg!(debug_assertions) {
		unsafe{ppn.clear_content();}
	}
	ppn
}

pub fn free_fs_page(ppn: PhysPageNum) {
	PAGE_ALLOCATOR.acquire().free(ppn, false)
}

pub fn claim_vm_page(to_claim: PhysPageNum) -> PageGuard {
	PAGE_ALLOCATOR.acquire().claim(to_claim, true);
	PageGuard::new(PageGuardInner::new(to_claim, true, false))
}

pub fn claim_fs_page(to_claim: PhysPageNum) -> PageGuard {
	PAGE_ALLOCATOR.acquire().claim(to_claim, false);
	PageGuard::new(PageGuardInner::new(to_claim, false, false))
}

pub fn stat_mem() -> (usize, usize) {
	PAGE_ALLOCATOR.acquire().stat()
}