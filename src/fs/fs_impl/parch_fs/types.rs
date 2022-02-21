use crate::{fs::{File, vfs::OpenMode, RegularFile}, mem::{PageGuard, PhysAddr, VirtPageNum, SCHEDULER_MEM_LAYOUT, VMASegment}, utils::{SpinMutex, ErrorNum, Mutex}, interrupt::get_cpu};
use super::{DIRECT_BLOCK_COUNT, INODE_SIZE, DENTRY_NAME_LEN, DENTRY_SIZE, fs::ParchFS};



use alloc::{sync::{Weak, Arc}};

use static_assertions::*;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct INodeNo(pub u32);

impl From<u32> for INodeNo {
    fn from(no: u32) -> Self {
        Self(no)
    }
}

impl INodeNo {
    pub fn to_pa(&self, fs: Weak<ParchFS>) -> PhysAddr {
        fs.upgrade().as_mut().unwrap().inodeno_2_pa(*self)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BlockNo(pub u32);

impl From<u32> for BlockNo {
    fn from(no: u32) -> Self {
        Self(no)
    }
}

impl BlockNo {
    pub fn to_pa(&self, fs: Weak<ParchFS>) -> PhysAddr {
        fs.upgrade().as_mut().unwrap().blockno_2_pa(*self)
    }
}

#[repr(u16)]
pub enum PFSPerm {
    OwnerR = 0400,
    OwnerW = 0200,
    OwnerX = 0100,
    GroupR = 0040,
    GroupW = 0020,
    GroupX = 0010,
    OtherR = 0004,
    OtherW = 0002,
    OtherX = 0001,
}

#[repr(u16)]
pub enum PFSType {
    SOCKET  = 0001,
    LINK    = 0002,
    REGULAR = 0004,
    BLOCK   = 0010,
    DIR     = 0020,
    CHAR    = 0040,
    FIFO    = 0100,    
}

#[repr(C)]
pub struct INode {
    permission          : PFSPerm,
    f_type              : PFSType,
    uid                 : u32,
    gid                 : u32,
    f_size              : u32,
    access_time         : u64,
    change_time         : u64,
    create_time         : u64,
    flags               : u32,
    hard_link_count     : u32,
    direct_blk_no       : [BlockNo; DIRECT_BLOCK_COUNT],
    indirect_blk        : BlockNo,
    indirect_blk2       : BlockNo,
    reserved            : [u8; 136]
}

assert_eq_size!(INode, [u8; INODE_SIZE]);

#[repr(C)]
pub struct DEntry {
    inode       : INodeNo,
    permission  : PFSPerm,
    f_type      : PFSType,
    name_len    : u16,
    f_name      : [u8; DENTRY_NAME_LEN]
}

assert_eq_size!(DEntry, [u8; DENTRY_SIZE]);

#[repr(C)]
pub struct SuperBlock {
    magic               : u64,
    xregs               : [u64; 31],
    base_kernel_satp    : u64,
    inode_count         : u64,
    block_count         : u64,
    free_inode          : u64,
    free_block          : u64,
    last_access         : u64,
    root_inode          : u32,
    reserved            : [u8; 3788]
}

pub struct PFSRegularInner {
    pub inode_no: INodeNo,
    pub cursor_pos: usize,
    pub open_mode: OpenMode,
    pub mmap_start: VirtPageNum,
    pub fs: Weak<ParchFS>
}

pub struct PFSRegular(SpinMutex<PFSRegularInner>);

pub struct PFSDirInner {
    inode_no: INodeNo,
    open_mode: OpenMode,
    fs: Weak<ParchFS>
}
pub struct PFSDir(SpinMutex<PFSDirInner>);

pub struct PFSLinkInner {
    inode_no: INodeNo,
    open_mode: OpenMode,
    wrapped: Arc<dyn File>,
    fs: Weak<ParchFS>
}
pub struct PFSLink(SpinMutex<PFSLinkInner>);

impl PFSRegular {
    pub fn new(inode_no: INodeNo, open_mode: OpenMode, fs: Weak<ParchFS>) -> Result<Arc::<Self>, ErrorNum> {
        let cpu = get_cpu();
        let cpu_inner = cpu.acquire();
        if let Some(pcb) = &cpu_inner.pcb {
            let mut pcb_inner = pcb.get_inner();
            let mem_layout = &mut pcb_inner.mem_layout;
            let res = Arc::new(Self ( SpinMutex::new("pfs file", PFSRegularInner{
                inode_no,
                cursor_pos: 0,
                mmap_start: 0.into(),
                open_mode,
                fs,
            })));
            let len = res.length();
            let start = mem_layout.get_space(len)?;
            let vma_segment = VMASegment::new_at(start, len, 0, res.clone(), open_mode.into())?;
            mem_layout.add_segment(vma_segment);
            mem_layout.do_map();
            Ok(res)
        } else {
            drop(cpu_inner);
            let mut mem_layout = SCHEDULER_MEM_LAYOUT.acquire();
            let res = Arc::new(Self ( SpinMutex::new("pfs file", PFSRegularInner{
                inode_no,
                cursor_pos: 0,
                mmap_start: 0.into(),
                open_mode,
                fs,
            })));
            let len = res.length();
            let start = mem_layout.get_space(len)?;
            let vma_segment = VMASegment::new_at(start, len, 0, res.clone(), open_mode.into())?;
            mem_layout.add_segment(vma_segment);
            mem_layout.do_map();
            Ok(res)
        }
    }

    pub fn get_blockno(&self, _offset: usize, _create: bool) -> Result<BlockNo, ErrorNum> {
        let _inner = self.0.acquire();
        todo!()
    }
}

impl File for PFSRegular {
    fn write            (&self, _data: alloc::vec::Vec::<u8>, _offset: usize) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn read             (&self, _length: usize, _offset: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        todo!()
    }

    fn as_socket    <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::SocketFile   + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_link      <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::LinkFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_regular   <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::RegularFile  + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_block     <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::BlockFile    + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_dir       <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::DirFile      + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_char      <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::CharFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_fifo      <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::FIFOFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_file      <'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        todo!()
    }

    fn vfs              (&self) -> Arc<dyn crate::fs::VirtualFileSystem> {
        todo!()
    }

    fn stat             (&self) -> crate::fs::types::FileStat {
        todo!()
    }
}

impl RegularFile for PFSRegular {
    fn get_page(&self, _offset: usize) -> Result<PageGuard, ErrorNum> {
        todo!()
    }
}

impl PFSRegular {
    pub fn length(&self) -> usize {
        todo!()
    }
}