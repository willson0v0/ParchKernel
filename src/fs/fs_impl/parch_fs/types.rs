use crate::{mem::{PhysAddr}, utils::{SpinMutex, Mutex, ErrorNum, time::get_real_time_epoch}, fs::{RegularFile, File, BlockFile, DirFile, OpenMode, types::{FileType, Permission, Dirent}, Cursor, LinkFile}};
use super::{DIRECT_BLK_COUNT, INODE_SIZE, DENTRY_NAME_LEN, DENTRY_SIZE, fs::{ParchFS}, PFSBase, BAD_BLOCK, BAD_INODE};

use core::mem::size_of;
use core::slice::from_raw_parts;
use bitflags::*;
use core::fmt::Debug;


use alloc::{string::{String}, sync::{Weak, Arc}, vec::Vec};

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
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
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
    pub fn to_pa(&self) -> PhysAddr {
        ParchFS::blockno_2_pa(*self)
    }

    pub fn clear_blk(&self) {
        unsafe {ParchFS::blockno_2_ppn(*self).clear_content()}
    }
}

bitflags! {
    #[repr(C)]
    pub struct PFSPerm: u16 {
        const OWNER_R = 0o400;
        const OWNER_W = 0o200;
        const OWNER_X = 0o100;
        const GROUP_R = 0o040;
        const GROUP_W = 0o020;
        const GROUP_X = 0o010;
        const OTHER_R = 0o004;
        const OTHER_W = 0o002;
        const OTHER_X = 0o001;
    }
}

enum_with_tryfrom_u16!(
    #[repr(u16)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PFSType {
        SOCKET  = 0o001,
        LINK    = 0o002,
        REGULAR = 0o004,
        BLOCK   = 0o010,
        DIR     = 0o020,
        CHAR    = 0o040,
        FIFO    = 0o100,
        UNKNOWN = 0o200,
        MOUNT   = 0o400,
    }
);

impl Into<Permission> for PFSPerm {
    fn into(self) -> Permission {
        Permission::from_bits(self.bits()).unwrap()
    }
}

impl Into<FileType> for PFSType {
    fn into(self) -> FileType {
        FileType::try_from(self as u16).expect(format!("unknown file type {}", self as u16).as_str())
    }
}

impl From<Permission> for PFSPerm {
    fn from(source: Permission) -> Self {
        Self::from_bits(source.bits()).unwrap()
    }
}

impl From<FileType> for PFSType {
    fn from(source: FileType) -> Self {
        Self::try_from(source as u16).expect(format!("unknown file type {}", source as u16).as_str())
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
    pub mount_info          : [u8; 16], // unused.
    pub reserved            : [u8; 112]
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
        let res = String::from_utf8(self.f_name[0..(self.name_len as usize)].to_vec()).unwrap();
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
    pub base: PFSBase,
    pub cursor: Cursor,
}

pub struct PFSRegular(SpinMutex<PFSRegularInner>);

impl Debug for PFSRegular {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("PFSRegular File @ {:?}", inner.base.path))
    }
}

impl Drop for PFSRegular {
    fn drop(&mut self) {
        // do nothing
    }
}

impl File for PFSRegular {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        let mut inner = self.0.acquire();
        let len = data.len();
        inner.base.write(data, inner.cursor)?;
        inner.cursor.0 += len;
        Ok(len)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        let mut inner = self.0.acquire();
        let res = inner.base.read(length, inner.cursor)?;
        inner.cursor.0 += res.len();
        Ok(res)
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

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        self.0.acquire().base.vfs()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, ErrorNum> {
        self.0.acquire().base.stat()
    }
}

impl RegularFile for PFSRegular {
    fn copy_page(&self, offset: usize) -> Result<crate::mem::PageGuard, crate::utils::ErrorNum> {
        self.0.acquire().base.copy_page(offset)
    }

    fn get_page(&self, offset: usize) -> Result<crate::mem::PageGuard, crate::utils::ErrorNum> {
        self.0.acquire().base.get_page(offset)
    }

    fn seek(&self, mut offset: usize) -> Result<usize, ErrorNum> {
        let mut inner = self.0.acquire();
        let len = inner.base.stat().unwrap().file_size;
        if offset > len {
            offset = len;
        }
        inner.cursor.0 = offset;
        Ok(inner.cursor.0)
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

impl Debug for PFSDir {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("PFSDir File @ {:?}", inner.base.path))
    }
}

impl PFSDirInner {
    fn read_dirent_raw(&self) -> Result<alloc::vec::Vec<PFSDEntry>, ErrorNum> {
        let stat = self.base.stat()?;
        if stat.file_size % size_of::<PFSDEntry>() != 0 {
            panic!("Malformed FS")
        }
        let dirent_count = stat.file_size / size_of::<PFSDEntry>();
        let buffer = self.base.read(stat.file_size, Cursor::at_start())?;
        let buffer = buffer.as_ptr() as *mut PFSDEntry;
        let buffer = unsafe{from_raw_parts(buffer, dirent_count).to_vec()};
        Ok(buffer)
    }

    fn write_dirent_at(&self, dirent: PFSDEntry, pos: usize) -> Result<(), ErrorNum> {
        let stat = self.base.stat()?;
        if stat.file_size % size_of::<PFSDEntry>() != 0 {
            panic!("Malformed FS")
        }
        if (pos + 1) * size_of::<PFSDEntry>() > stat.file_size {
            panic!("Dirent out of bound")
        }
        // reset stat
        let buffer: *const PFSDEntry = &dirent;
        let buffer = buffer as *const u8;
        let buffer = unsafe{from_raw_parts(buffer, size_of::<PFSDEntry>()).to_vec()};
        self.base.write(buffer, Cursor(pos * size_of::<PFSDEntry>()))?;
        Ok(())
    }

    fn add_dirent(&self, dirent: PFSDEntry) -> Result<(), ErrorNum> {
        let dirents = self.read_dirent_raw()?;
        let mut empty_dirent = None;
        for (idx, d) in dirents.iter().enumerate() {
            if d.inode == BAD_INODE {
                empty_dirent = Some(idx);
                break;
            }
        }
        if empty_dirent.is_none() {
            empty_dirent = Some(dirents.len());
            self.base.expand((dirents.len() + 1) * size_of::<PFSDEntry>())?;
        }
        let pos = empty_dirent.unwrap() as usize;
        self.write_dirent_at(dirent, pos)
    }

    fn remove_self(&self) {
        let entries = self.read_dirent_raw().unwrap();
        let mut children_dir: Vec<PFSDir> = Vec::new();
        for (idx, e) in entries.iter().enumerate() {
            if e.inode != BAD_INODE {
                let fs = self.base.fs.upgrade().unwrap();
                let mut fs_inner = fs.inner.acquire();
                let inode_guard = fs_inner.get_inode(e.inode.into()).unwrap();
                let mut inode = inode_guard.acquire();
                inode.hard_link_count -= 1;
                if inode.hard_link_count == 0 {
                    if inode.f_type == PFSType::DIR {
                        children_dir.push(PFSDir(SpinMutex::new("PFS", PFSDirInner{
                            base: PFSBase{
                                inode_no: e.inode,
                                open_mode: OpenMode::SYS,
                                fs: self.base.fs.clone(),
                                path: self.base.path.append(e.name()).unwrap(),
                            }})
                        ));
                        // keep the inode and free after it's children are freed.
                    } else {
                        let base = PFSBase{
                            inode_no: e.inode,
                            open_mode: OpenMode::SYS,
                            fs: self.base.fs.clone(),
                            path: self.base.path.append(e.name()).unwrap(),
                        };
                        base.resize_locked(0, &mut fs_inner, &mut inode).unwrap();
                        fs_inner.free_inode(e.inode.into());    
                    }
                }
                drop(inode);
                drop(inode_guard);
                drop(fs_inner);
                self.write_dirent_at(PFSDEntry::empty(), idx).unwrap();
            }
        }
        for c in children_dir {
            c.0.acquire().remove_self();
        }
        self.base.resize(0).unwrap();
        self.base.fs.upgrade().unwrap().inner.acquire().free_inode(self.base.inode_no);
    }
}

impl File for PFSDir {
    fn write(&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EISDIR)
    }

    fn read(&self, _length: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
        Err(ErrorNum::EISDIR)
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

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        self.0.acquire().base.vfs()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, ErrorNum> {
        self.0.acquire().base.stat()
    }
}

impl DirFile for PFSDir {
    fn open_entry(&self, entry_name: &String, mode: OpenMode) -> Result<alloc::sync::Arc<dyn File>, ErrorNum> {
        let entries = self.read_dirent()?;
        let inner = self.0.acquire();
        for e in &entries {
            // verbose!("Opendir looking for {}, f_type {:?}, target {}", e.f_name, e.f_type, rel_path.components[0]);
            if e.f_name.eq(entry_name) {
                let base = PFSBase::new(
                    e.inode.into(), 
                    inner.base.path.append(e.f_name.clone())?,
                    mode,
                    inner.base.fs.clone()
                )?;
                let f_type = base.f_type()?;
                let inode = inner.base.fs.upgrade().unwrap().get_inode(e.inode.into())?;
                let mut inode_inner = inode.acquire();
                inode_inner.access_time = get_real_time_epoch();
                let res: Arc<dyn File> = match f_type {
                    FileType::REGULAR => {
                        Arc::new(PFSRegular(SpinMutex::new("PFSFile lock", PFSRegularInner{base, cursor: Cursor(0)})))
                    },
                    FileType::DIR => {
                        Arc::new(PFSDir(SpinMutex::new("PFSFile lock", PFSDirInner{base})))
                    },
                    FileType::LINK => {
                        Arc::new(PFSLink(SpinMutex::new("PFSFile lock", PFSLinkInner{base})))
                    },
                    _ => {
                        panic!("Malformed fs, bad type")
                    }
                };
                return Ok(res);
            }
        }
        if mode.contains(OpenMode::CREATE) {
            // default to create regular file
            drop(inner);
            self.make_file(entry_name.clone(), Permission::default(), FileType::REGULAR)?;
            self.open_entry(&entry_name, mode)
        } else {
            Err(ErrorNum::ENOENT)
        }
    }

    fn make_file(&self, name: String, perm: Permission, f_type: FileType) -> Result<Arc<dyn File>, ErrorNum>{
        if f_type != FileType::REGULAR && f_type != FileType::DIR {
            return Err(ErrorNum::EBADTYPE);
        }
        if name.bytes().len() > DENTRY_NAME_LEN {
            return Err(ErrorNum::ENAMETOOLONG);
        }
        let dirents = self.read_dirent()?;
        for d in dirents {
            if d.f_name == name {
                return Err(ErrorNum::EEXIST);
            }
        }
        
        let inner = self.0.acquire();
        let parent_inode = inner.base.inode_no;
        let fs = inner.base.fs.upgrade().unwrap();
        let mut fs_inner = fs.inner.acquire();
        let inode_no = fs_inner.alloc_inode();
        let inode_guard = fs_inner.get_inode(inode_no)?;
        let mut inode = inode_guard.acquire();
        
        inode.permission = perm.into();
        inode.f_type = f_type.into();
        inode.uid = 0;
        inode.gid = 0;
        inode.flags = 0;
        inode.hard_link_count = if f_type == FileType::DIR {2} else {1};
        inode.direct_blk_no = [BAD_BLOCK; DIRECT_BLK_COUNT];
        inode.indirect_blk = BAD_BLOCK;
        inode.indirect_blk2 = BAD_BLOCK;
        inode.f_size = 0;
        inode.access_time = get_real_time_epoch();
        inode.change_time = get_real_time_epoch();
        inode.create_time = get_real_time_epoch();

        let bytes: Vec<u8> = name.bytes().collect();
        let mut f_name: [u8; DENTRY_NAME_LEN] = [0; DENTRY_NAME_LEN];
        f_name[0..bytes.len()].clone_from_slice(&bytes[..]) ;

        drop(inode);
        drop(inode_guard);
        drop(fs_inner);
        drop(fs);

        inner.add_dirent(PFSDEntry {
            inode: inode_no,
            permission: perm.into(),
            f_type: f_type.into(),
            name_len: bytes.len() as u16,
            f_name,
        })?;
        
        drop(inner);

        let res = self.open_entry(&name.into(), OpenMode::SYS)?;
        if let Ok(dir) = res.clone().as_dir() {
            let dir: Arc<PFSDir> = Arc::downcast(dir.as_any()).unwrap();
            let mut dot_name = [0u8; DENTRY_NAME_LEN];
            dot_name[0] = b'.';
            dir.0.acquire().add_dirent( PFSDEntry {
                    inode: inode_no,
                    permission: perm.into(),
                    f_type: PFSType::DIR,
                    name_len: 1,
                    f_name: dot_name,
                }
            ).unwrap();
            
            let mut dot2_name = [0u8; DENTRY_NAME_LEN];
            dot2_name[0] = b'.';
            dot2_name[1] = b'.';
            dir.0.acquire().add_dirent( PFSDEntry {
                    inode: parent_inode,
                    permission: perm.into(),
                    f_type: PFSType::DIR,
                    name_len: 2,
                    f_name: dot2_name,
                }
            ).unwrap();
        }
        Ok(res)
    }

    fn remove_file(&self, name: String) -> Result<(), ErrorNum> {
        let entries = self.read_dirent()?;
        for (idx, e) in entries.iter().enumerate() {
            if e.f_name == name {
                let inner = self.0.acquire();
                let fs = inner.base.fs.upgrade().unwrap();
                let mut fs_inner = fs.inner.acquire();
                let inode_guard = fs_inner.get_inode(e.inode.into())?;
                let mut inode = inode_guard.acquire();
                if inode.f_type == PFSType::DIR {
                    let child_inner = PFSDirInner {
                        base: PFSBase {
                            inode_no: e.inode.into(),
                            open_mode: OpenMode::SYS,
                            fs: inner.base.fs.clone(),
                            path: inner.base.path.append(e.f_name.clone()).unwrap(),
                        }
                    };
                    drop(fs_inner);
                    drop(inode);
                    child_inner.remove_self();
                } else {
                    inode.hard_link_count -= 1;
                    if inode.hard_link_count == 0 {
                        let base = PFSBase {
                            inode_no: e.inode.into(),
                            open_mode: OpenMode::SYS,
                            fs: inner.base.fs.clone(),
                            path: inner.base.path.append(e.f_name.clone()).unwrap(),
                        };
                        base.resize_locked(0, &mut fs_inner, &mut inode).unwrap();
                        fs_inner.free_inode(e.inode.into());
                    }
                    drop(fs_inner);
                    drop(inode);
                }
                inner.write_dirent_at(PFSDEntry::empty(), idx)?;
                return Ok(());
            }
        }
        Err(ErrorNum::ENOENT)
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<Dirent>, ErrorNum> {
        let mut res = self.0.acquire().read_dirent_raw()?;
        res.retain(|&x| x.inode != BAD_INODE);
        Ok(res.iter().map(|&x| x.into()).collect())
    }
}

pub struct PFSLinkInner {
    pub base: PFSBase
}
pub struct PFSLink(pub SpinMutex<PFSLinkInner>);

impl Drop for PFSLink {
    fn drop(&mut self) {
        todo!()
    }
}

impl Debug for PFSLink {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("PFSLink File @ {:?}", inner.base.path))
    }
}

impl File for PFSLink {
    fn write            (&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, ErrorNum> {
        todo!()
    }

    fn read             (&self, _length: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
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

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }
}

impl LinkFile for PFSLink {
    fn read_link(&self) -> Result<crate::fs::Path, ErrorNum> {
        todo!()
    }

    fn write_link(&self, _path: &crate::fs::Path) -> Result<(), ErrorNum> {
        todo!()
    }
}