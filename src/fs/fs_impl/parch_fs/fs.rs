use alloc::{collections::BTreeMap, sync::Arc};

use crate::{fs::{VirtualFileSystem, File, Path, fs_impl::parch_fs::{INODE_SIZE}}, utils::{SpinMutex}, mem::{BitMap, PhysAddr}, config::PAGE_SIZE};

use super::{INode, INodeNo, SuperBlock, BlockNo};

pub struct ParchFSInner {
    opened_files: BTreeMap<Path, Arc<dyn File>>,
    superblock: &'static SuperBlock,
    // no fs_bitmap/mm_bitmap, mem module take care of that
    inode_bitmap: BitMap
}

pub struct ParchFS(SpinMutex<ParchFSInner>);

impl ParchFS {
    pub fn inodeno_2_pa(&self, inode_no: INodeNo) -> PhysAddr {
        extern "C" {fn INODE_LIST_ADDRESS();}
        PhysAddr::from(INODE_LIST_ADDRESS as usize) + INODE_SIZE * (inode_no.0 as usize)
    }

    pub fn blockno_2_pa(&self, block_no: BlockNo) -> PhysAddr {
        extern "C" {fn BASE_ADDRESS();}
        PhysAddr::from(BASE_ADDRESS as usize) + PAGE_SIZE * (block_no.0 as usize)
    }

    pub fn get_inode(&self, inode_no: INodeNo) -> &'static mut INode {
        let pa = self.inodeno_2_pa(inode_no);
        unsafe{pa.instantiate_volatile()}
    }
}

impl VirtualFileSystem for ParchFS {
    fn open(&self, _path: &crate::fs::Path, _mode: crate::fs::vfs::OpenMode) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        todo!()
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