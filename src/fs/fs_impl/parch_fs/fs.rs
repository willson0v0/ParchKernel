use core::fmt::Debug;

use alloc::{collections::{BTreeMap}, sync::Arc};

use crate::{fs::{VirtualFileSystem, fs_impl::{parch_fs::{INODE_SIZE, BLK_SIZE, PFS_MAGIC, INODE_BITMAP_SIZE, PFSDir, PFSBase}, PARCH_FS}, DirFile, OpenMode, Path}, utils::{SpinMutex, Mutex, ErrorNum}, mem::{BitMap, PhysAddr, alloc_fs_page, free_fs_page, PhysPageNum}, config::PAGE_SIZE};

use super::{PFSINode, INodeNo, SuperBlock, BlockNo};

pub struct ParchFSInner {
    // lock inode, not locking file (user's task)
    inode_locks: BTreeMap<INodeNo, Arc<SpinMutex<&'static mut PFSINode>>>,
    superblock: &'static mut SuperBlock,    // don't need additional lock, ParchFSInner's mutex took care of that.
    // no fs_bitmap/mm_bitmap, mem module take care of that
    // XXX: move them here? multiple ParchFS in main NVM?
    inode_bitmap: BitMap
}

pub struct ParchFS(pub SpinMutex<ParchFSInner>);

impl Debug for ParchFS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ParchFS struct")
    }
}

impl ParchFS {
    pub fn new() -> Self {
        Self(SpinMutex::new("PFS lock", ParchFSInner::new()))
    }

    pub fn inodeno_2_pa(inode_no: INodeNo) -> PhysAddr {
        extern "C" {fn INODE_LIST_ADDRESS();}
        PhysAddr::from(INODE_LIST_ADDRESS as usize) + INODE_SIZE * (inode_no.0 as usize)
    }

    pub fn blockno_2_pa(block_no: BlockNo) -> PhysAddr {
        extern "C" {fn BASE_ADDRESS();}
        PhysAddr::from(BASE_ADDRESS as usize) + PAGE_SIZE * (block_no.0 as usize)
    }

    pub fn pa_2_blockno(pa: PhysAddr) -> BlockNo {
        extern "C" {fn BASE_ADDRESS();}
        assert!(pa.0 % BLK_SIZE == 0, "PA not aligned");
        BlockNo::from((pa - PhysAddr::from(BASE_ADDRESS as usize)) / BLK_SIZE)
    }

    pub fn ppn_2_blockno(ppn: PhysPageNum) -> BlockNo {
        extern "C" {fn BASE_ADDRESS();}
        BlockNo::from(PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize)) - ppn)
    }

    pub fn blockno_2_ppn(block_no: BlockNo) -> PhysPageNum {
        extern "C" {fn BASE_ADDRESS();}
        PhysPageNum::from(PhysAddr::from(BASE_ADDRESS as usize)) + (block_no.0 as usize)
    }

    /// FIXME: Maybe a custom struct for Arc<SpinMutex<&'static mut INode>>, then implement Drop for auto recover?
    /// Calculate how much extra space it need
    /// !!! MUST NOT USE RAW instantiate_volatile(), for one INode correspond to multiple File and File Mutex is not enough
    pub fn get_inode(&self, inode_no: INodeNo) -> Result<Arc<SpinMutex<&'static mut PFSINode>>, ErrorNum> {
        let mut inner = self.0.acquire();
        inner.get_inode(inode_no)
    }

    pub fn alloc_blk(&self) -> BlockNo {
        let mut inner = self.0.acquire();
        inner.alloc_blk()
    }

    pub fn free_blk(&self, block_no: BlockNo) {
        let mut inner = self.0.acquire();
        inner.free_blk(block_no);
    }
}

impl ParchFSInner {
    pub fn new() -> Self {
        extern "C" {
            fn INODE_BITMAP_ADDRESS();
            fn SUPERBLOCK_ADDRESS();
        }
        let inode_bitmap_start = PhysAddr::from(INODE_BITMAP_ADDRESS as usize);
        let superblock_start = PhysAddr::from(SUPERBLOCK_ADDRESS as usize);
        let superblock: &mut SuperBlock = unsafe{superblock_start.instantiate_volatile()};

        let res = Self {
            inode_locks: BTreeMap::new(),
            superblock,
            inode_bitmap: BitMap::new(inode_bitmap_start, INODE_BITMAP_SIZE)
        };
        assert!(res.superblock.magic == PFS_MAGIC, "Bad FS Magic");
        res
    }

    /// FIXME: Maybe a custom struct for Arc<SpinMutex<&'static mut INode>>, then implement Drop for auto recover?
    /// Calculate how much extra space it need
    /// !!! MUST NOT USE RAW instantiate_volatile(), for one INode correspond to multiple File and File Mutex is not enough
    /// if holding lock of PFSInner, use this function instead of outer wrappers' function to avoid deadlock
    pub fn get_inode(&mut self, inode_no: INodeNo) -> Result<Arc<SpinMutex<&'static mut PFSINode>>, ErrorNum> {
        if self.inode_bitmap.get(inode_no.0 as usize) == false {
            // remove lock
            self.inode_locks.remove(&inode_no);
            // prevent summon it again
            return Err(ErrorNum::ENOENT);
        }
        if self.inode_locks.contains_key(&inode_no) {
            Ok(self.inode_locks.get(&inode_no).unwrap().clone())
        } else {
            let pa = ParchFS::inodeno_2_pa(inode_no);
            let inode: &mut PFSINode = unsafe{pa.instantiate_volatile()};
            let mutex = Arc::new(SpinMutex::new("INode lock", inode));
            self.inode_locks.insert(inode_no, mutex.clone());
            return Ok(mutex);
        }
    }

    pub fn alloc_blk(&mut self) -> BlockNo {
        self.superblock.free_block -= 1;
        let pa = alloc_fs_page();
        ParchFS::pa_2_blockno(pa.into())
    }

    pub fn free_blk(&mut self, block_no: BlockNo) {
        self.superblock.free_block += 1;
        let ppn = ParchFS::blockno_2_ppn(block_no);
        free_fs_page(ppn)
    }

    pub fn alloc_inode(&mut self) -> INodeNo {
        let inode_no = self.inode_bitmap.first_empty().unwrap();
        self.inode_bitmap.set(inode_no);
        inode_no.into()
    }

    pub fn free_inode(&mut self, inode_no: INodeNo) {
        let inode_no = inode_no.0 as usize;
        assert!(self.inode_bitmap.get(inode_no), "Freeing free inode");
        self.inode_bitmap.clear(inode_no);
    }
}

impl VirtualFileSystem for ParchFS {
    fn open(&self, path: &crate::fs::Path, mode: crate::fs::vfs::OpenMode) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        let root_dir = Arc::new(
            PFSDir(SpinMutex::new("PFS ROOT", crate::fs::fs_impl::parch_fs::PFSDirInner { base: PFSBase {
                inode_no: self.0.acquire().superblock.root_inode.into(),
                open_mode: OpenMode::SYS,
                mmap_start: None,
                fs: Arc::downgrade(&PARCH_FS),
                path: Path::root(),
            } }))
        );
        root_dir.open_dir(path, mode)
    }

    fn mkdir(&self, _path: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile>, crate::utils::ErrorNum> {
        todo!()
    }

    fn mkfile(&self, _path: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile>, crate::utils::ErrorNum> {
        todo!()
    }

    fn remove(&self, _path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn link(&self, _dest: alloc::sync::Arc<dyn crate::fs::File>, _link_file: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        todo!()
    }

    fn sym_link(&self, _abs_src: &crate::fs::Path, _rel_dst: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile>, crate::utils::ErrorNum> {
        todo!()
    }
}