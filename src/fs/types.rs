use core::fmt::Debug;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use crate::mem::{PageGuard, MemLayout, VirtPageNum};
use crate::utils::ErrorNum;

use super::vfs::OpenMode;
use super::{VirtualFileSystem, Path};
use bitflags::*;

#[derive(Debug, Clone)]
pub struct FileStat {
    pub open_mode   : OpenMode,
    pub file_size   : usize,
    pub path        : Path,
    pub inode       : u32,
    pub fs          : Weak<dyn VirtualFileSystem>,
}

#[derive(Debug, Clone)]
pub struct Dirent {
    pub inode       : u32,
    pub permission  : Permission,
    pub f_type      : FileType,
    pub f_name      : String
}


bitflags! {
    pub struct Permission: u16 {
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

impl Permission {
    pub fn default() -> Self {
        Self::OwnerR | Self::OwnerW | Self::GroupR | Self::OtherR
    }
}

bitflags! {
    pub struct FileType: u16 {
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

#[derive(Debug, Clone)]
pub struct DEntry {
    permission: Permission,
    file_type: FileType,
    name: String
}

pub trait File: Send + Sync + Drop + Debug {
    fn write            (&self, data: Vec::<u8>, offset: usize) -> Result<(), ErrorNum>;
    fn read             (&self, length: usize, offset: usize) -> Result<Vec<u8>, ErrorNum>;
    fn as_socket    <'a>(self: Arc<Self>) -> Result<Arc<dyn SocketFile   + 'a>, ErrorNum> where Self: 'a;
    fn as_link      <'a>(self: Arc<Self>) -> Result<Arc<dyn LinkFile     + 'a>, ErrorNum> where Self: 'a;
    fn as_regular   <'a>(self: Arc<Self>) -> Result<Arc<dyn RegularFile  + 'a>, ErrorNum> where Self: 'a;
    fn as_block     <'a>(self: Arc<Self>) -> Result<Arc<dyn BlockFile    + 'a>, ErrorNum> where Self: 'a;
    fn as_dir       <'a>(self: Arc<Self>) -> Result<Arc<dyn DirFile      + 'a>, ErrorNum> where Self: 'a;
    fn as_char      <'a>(self: Arc<Self>) -> Result<Arc<dyn CharFile     + 'a>, ErrorNum> where Self: 'a;
    fn as_fifo      <'a>(self: Arc<Self>) -> Result<Arc<dyn FIFOFile     + 'a>, ErrorNum> where Self: 'a;
    fn as_file      <'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a;
    fn vfs              (&self) -> Arc<dyn VirtualFileSystem>;
    fn stat             (&self) -> Result<FileStat, ErrorNum>;
}

pub trait SocketFile    : File {}
pub trait LinkFile      : File {}
pub trait RegularFile   : File {
    fn get_page(&self, offset: usize) -> Result<PageGuard, ErrorNum>;
    fn register_mmap(self: Arc<Self>, mem_layout: &mut MemLayout) -> Result<VirtPageNum, ErrorNum>;
}
pub trait BlockFile     : File {}
pub trait DirFile       : File {
    fn open_dir(&self, rel_path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum>;
    fn make_file(&self, name: String, perm: Permission, f_type: FileType) -> Result<Arc<dyn File>, ErrorNum>;
    fn remove_file(&self, name: String) -> Result<(), ErrorNum>;
    fn read_dirent(&self) -> Result<Vec<Dirent>, ErrorNum>;
}
pub trait CharFile      : File {}
pub trait FIFOFile      : File {}