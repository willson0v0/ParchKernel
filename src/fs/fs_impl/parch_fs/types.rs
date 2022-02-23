use crate::{mem::{PhysAddr, VMASegment}, utils::{SpinMutex, Mutex, ErrorNum}, fs::{RegularFile, File, BlockFile, DirFile, OpenMode, types::{FileType, Permission, Dirent}}};
use super::{DIRECT_BLK_COUNT, INODE_SIZE, DENTRY_NAME_LEN, DENTRY_SIZE, fs::{ParchFS}, PFSBase, BAD_BLOCK, BAD_INODE};

use core::mem::size_of;
use core::slice::from_raw_parts;
use bitflags::*;



use alloc::{sync::{Weak, Arc}, string::String};

use static_assertions::*;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct INodeNo(pub u32);

impl From<u32> for INodeNo {
    fn from(no: u32) -> Self {
        Self(no)
    }
}


impl From<usize> for INodeNo {
    fn from(no: usize) -> Self {
        Self(no as u32)
    }
}

impl INodeNo {
    pub fn to_pa(&self, _fs: Weak<ParchFS>) -> PhysAddr {
        ParchFS::inodeno_2_pa(*self)
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockNo(pub u32);

impl From<u32> for BlockNo {
    fn from(no: u32) -> Self {
        Self(no)
    }
}

impl From<usize> for BlockNo {
    fn from(no: usize) -> Self {
        Self(no as u32)
    }
}

impl BlockNo {
    pub fn to_pa(&self, _fs: Weak<ParchFS>) -> PhysAddr {
        ParchFS::blockno_2_pa(*self)
    }
}

bitflags! {
    pub struct PFSPerm: u16 {
        const OwnerR = 0400;
        const OwnerW = 0200;
        const OwnerX = 0100;
        const GroupR = 0040;
        const GroupW = 0020;
        const GroupX = 0010;
        const OtherR = 0004;
        const OtherW = 0002;
        const OtherX = 0001;
    }
}

bitflags! {
    pub struct PFSType: u16 {
        const SOCKET  = 0001;
        const LINK    = 0002;
        const REGULAR = 0004;
        const BLOCK   = 0010;
        const DIR     = 0020;
        const CHAR    = 0040;
        const FIFO    = 0100;
        const UNKNOWN = 0200;
    }
}

impl Into<Permission> for PFSPerm {
    fn into(self) -> Permission {
        Permission::from_bits(self.bits()).unwrap()
    }
}

impl Into<FileType> for PFSType {
    fn into(self) -> FileType {
        FileType::from_bits(self.bits()).unwrap()
    }
}

impl From<Permission> for PFSPerm {
    fn from(source: Permission) -> Self {
        Self::from_bits(source.bits()).unwrap()
    }
}

impl From<FileType> for PFSType {
    fn from(source: FileType) -> Self {
        Self::from_bits(source.bits()).unwrap()
    }
}

/// NEVER DERIVE COPY/CLONE, inode stay in the original pos
#[repr(C)]
pub struct PFSINode {
    pub permission          : PFSPerm,
    pub f_type              : PFSType,
    pub uid                 : u32,
    pub gid                 : u32,
    pub flags               : u32,
    pub hard_link_count     : u32,
    pub direct_blk_no       : [BlockNo; DIRECT_BLK_COUNT],
    pub indirect_blk        : BlockNo,
    pub indirect_blk2       : BlockNo,
    pub f_size              : usize,
    pub access_time         : usize,
    pub change_time         : usize,
    pub create_time         : usize,
    pub reserved            : [u8; 128]
}

assert_eq_size!(PFSINode, [u8; INODE_SIZE]);

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PFSDEntry {
    inode       : INodeNo,
    permission  : PFSPerm,
    f_type      : PFSType,
    name_len    : u16,
    f_name      : [u8; DENTRY_NAME_LEN]
}

assert_eq_size!(PFSDEntry, [u8; DENTRY_SIZE]);

impl PFSDEntry {
    pub fn name(&self) -> String {
        let res = String::from_utf8(self.f_name.to_vec()).unwrap();
        res.chars().filter(|&x| x != '\0').collect()
    }

    pub fn empty() -> Self {
        Self {
            inode: BAD_INODE,
            permission: PFSPerm::empty(),
            f_type: PFSType::UNKNOWN,
            name_len: 0,
            f_name: [0; DENTRY_NAME_LEN],
        }
    }
}

impl Into<Dirent> for PFSDEntry {
    fn into(self) -> Dirent {
        Dirent { 
            inode: self.inode.0, 
            permission: self.permission.into(), 
            f_type: self.f_type.into(), 
            f_name: self.name()
        }
    }
}

#[repr(C)]
pub struct SuperBlock {
    pub magic               : u64,
    pub xregs               : [u64; 31],
    pub base_kernel_satp    : u64,
    pub inode_count         : u64,
    pub block_count         : u64,
    pub free_inode          : u64,
    pub free_block          : u64,
    pub last_access         : u64,
    pub root_inode          : u32,
    pub reserved            : [u8; 3788]
}

pub struct PFSRegularInner {
    pub base: PFSBase
}

pub struct PFSRegular(SpinMutex<PFSRegularInner>);

impl Drop for PFSRegular {
    fn drop(&mut self) {
        // do nothing
    }
}

impl File for PFSRegular {
    fn write(&self, data: alloc::vec::Vec::<u8>, offset: usize) -> Result<(), crate::utils::ErrorNum> {
        self.0.acquire().base.write(data, offset)
    }

    fn read(&self, length: usize, offset: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        self.0.acquire().base.read(length, offset)
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile   + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn RegularFile  + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_block<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile    + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_dir<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile      + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_char<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        self.0.acquire().base.vfs()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, ErrorNum> {
        self.0.acquire().base.stat()
    }

    fn do_mmap(self: Arc<Self>, mem_layout: &mut crate::mem::MemLayout) -> Result<crate::mem::VirtPageNum, ErrorNum> {
        let stat = self.stat()?;
        let start_vpn = mem_layout.get_space(stat.file_size)?;
        mem_layout.add_segment(VMASegment::new_at(
            start_vpn,
            stat.file_size,
            0,
            self.clone(),
            stat.open_mode.into()
        )?);
        Ok(start_vpn)
    }
}

impl RegularFile for PFSRegular {
    fn get_page(&self, offset: usize) -> Result<crate::mem::PageGuard, crate::utils::ErrorNum> {
        self.0.acquire().base.get_page(offset)
    }
}

impl BlockFile for PFSRegular {}

pub struct PFSDirInner {
    pub base: PFSBase
}
pub struct PFSDir(pub SpinMutex<PFSDirInner>);

impl Drop for PFSDir {
    fn drop(&mut self) {
        // do nothing
    }
}

impl File for PFSDir {
    fn write(&self, data: alloc::vec::Vec::<u8>, offset: usize) -> Result<(), ErrorNum> {
        let inner = self.0.acquire();
        if inner.base.open_mode == OpenMode::SYS {
            inner.base.write(data, offset)
        } else {
            Err(ErrorNum::EISDIR)
        }
    }

    fn read(&self, length: usize, offset: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
        self.0.acquire().base.read(length, offset)
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile   + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile     + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn RegularFile  + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn BlockFile    + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile      + 'a>, ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_char<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile     + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile     + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        self.0.acquire().base.vfs()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, ErrorNum> {
        self.0.acquire().base.stat()
    }

    fn do_mmap          (self: Arc<Self>, _mem_layout: &mut crate::mem::MemLayout) -> Result<crate::mem::VirtPageNum, ErrorNum> {
        todo!()
    }
}

impl DirFile for PFSDir {
    fn open_dir(&self, rel_path: &crate::fs::Path, mode: OpenMode) -> Result<alloc::sync::Arc<dyn File>, ErrorNum> {
        let entries = self.read_dirent()?;
        let inner = self.0.acquire();
        for e in &entries {
            if e.f_name == rel_path.components[0] {
                let base = PFSBase::new(
                    e.inode.into(), 
                    inner.base.path.append(e.f_name.clone())?,
                    mode,
                    inner.base.fs.clone()
                )?;
                let f_type = base.f_type()?;
                if rel_path.len() == 1 {
                    let res: Arc<dyn File> = match f_type {
                        FileType::REGULAR => {
                            Arc::new(PFSRegular(SpinMutex::new("PFSFile lock", PFSRegularInner{base})))
                        },
                        FileType::DIR => {
                            Arc::new(PFSDir(SpinMutex::new("PFSFile lock", PFSDirInner{base})))
                        },
                        FileType::LINK => {
                            Arc::new(PFSLink(SpinMutex::new("PFSFile lock", PFSLinkInner{base, link_tgt: None})))
                        },
                        _ => {
                            panic!("Malformed fs, bad type")
                        }
                    };
                    return Ok(res);
                } else if f_type == FileType::DIR {
                    drop(inner);
                    return PFSDir(SpinMutex::new("PFSFile lock", PFSDirInner{base})).open_dir(&rel_path.strip_head(), mode);
                } else {
                    return Err(ErrorNum::ENOTDIR);
                }
            }
        }
        Err(ErrorNum::ENOENT)
    }

    fn make_file(&self, _name: String, perm: Permission, f_type: FileType) -> Result<(), ErrorNum>{
        if f_type != FileType::REGULAR || f_type != FileType::DIR {
            return Err(ErrorNum::EBADTYPE);
        }
        
        let inner = self.0.acquire();
        let fs = inner.base.fs.upgrade().unwrap();
        let mut fs_inner = fs.0.acquire();
        let inode_no = fs_inner.alloc_inode();
        let inode_guard = fs_inner.get_inode(inode_no)?;
        let mut inode = inode_guard.acquire();
        
        inode.permission = perm.into();
        inode.f_type = f_type.into();
        inode.uid = 0;
        inode.gid = 0;
        inode.flags = 0;
        inode.hard_link_count = 1;
        inode.direct_blk_no = [BAD_BLOCK; DIRECT_BLK_COUNT];
        inode.indirect_blk = BAD_BLOCK;
        inode.indirect_blk2 = BAD_BLOCK;
        inode.f_size = 0;
        inode.access_time = 0xbeef;
        inode.change_time = 0xbeef;
        inode.create_time = 0xbeef;

        return Ok(())
    }

    fn remove_file(&self, name: String) -> Result<(), ErrorNum> {
        let entries = self.read_dirent()?;
        for (idx, e) in entries.iter().enumerate() {
            if e.f_name == name {
                let inner = self.0.acquire();
                let fs = inner.base.fs.upgrade().unwrap();
                let mut fs_inner = fs.0.acquire();
                let inode_guard = fs_inner.get_inode(e.inode.into())?;
                let mut inode = inode_guard.acquire();
                inode.hard_link_count -= 1;
                if inode.hard_link_count == 0 {
                    fs_inner.free_inode(e.inode.into());
                }

                let offset = idx * size_of::<PFSDEntry>();
                let buffer = PFSDEntry::empty();
                let u8_buf = unsafe{from_raw_parts((&buffer as *const PFSDEntry) as *const u8, size_of::<PFSDEntry>())}.to_vec();
                inner.base.write(u8_buf, offset);
                return Ok(());
            }
        }
        Err(ErrorNum::ENOENT)
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<crate::fs::types::Dirent>, ErrorNum> {
        let inner = self.0.acquire();
        let stat = inner.base.stat()?;
        if stat.file_size % size_of::<PFSDEntry>() != 0 {
            panic!("Malformed FS")
        }
        let dirent_count = stat.file_size / size_of::<PFSDEntry>();
        let buffer = inner.base.read(stat.file_size, 0)?;
        let buffer = buffer.as_ptr() as *mut PFSDEntry;
        let mut buffer = unsafe{from_raw_parts(buffer, dirent_count).to_vec()};
        buffer.retain(|&dirent| dirent.inode != BAD_INODE);
        Ok(buffer.iter().map(|&dirent| dirent.into()).collect())
    }
}

pub struct PFSLinkInner {
    pub base: PFSBase,
    pub link_tgt: Option<Arc<dyn File>>
}
pub struct PFSLink(SpinMutex<PFSLinkInner>);

impl Drop for PFSLink {
    fn drop(&mut self) {
        todo!()
    }
}

impl File for PFSLink {
    fn write            (&self, _data: alloc::vec::Vec::<u8>, _offset: usize) -> Result<(), ErrorNum> {
        todo!()
    }

    fn read             (&self, _length: usize, _offset: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
        todo!()
    }

    fn as_socket    <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::SocketFile   + 'a>, ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_link      <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::LinkFile     + 'a>, ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_regular   <'a>(self: Arc<Self>) -> Result<Arc<dyn RegularFile  + 'a>, ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_block     <'a>(self: Arc<Self>) -> Result<Arc<dyn BlockFile    + 'a>, ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_dir       <'a>(self: Arc<Self>) -> Result<Arc<dyn DirFile      + 'a>, ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_char      <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::CharFile     + 'a>, ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_fifo      <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::FIFOFile     + 'a>, ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_file      <'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        todo!()
    }

    fn vfs              (&self) -> Arc<dyn crate::fs::VirtualFileSystem> {
        todo!()
    }

    fn stat             (&self) -> Result<crate::fs::types::FileStat, ErrorNum> {
        todo!()
    }

    fn do_mmap          (self: Arc<Self>, _mem_layout: &mut crate::mem::MemLayout) -> Result<crate::mem::VirtPageNum, ErrorNum> {
        todo!()
    }
}