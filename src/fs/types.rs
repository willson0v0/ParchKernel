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
    // TODO: uid/gid/times
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

impl Permission {
    pub fn default() -> Self {
        Self::OWNER_R | Self::OWNER_W | Self::GROUP_R | Self::OTHER_R
    }
}

enum_with_tryfrom_u16!(
    #[repr(u16)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum FileType {
        SOCKET  = 0o001,
        LINK    = 0o002,
        REGULAR = 0o004,
        BLOCK   = 0o010,
        DIR     = 0o020,
        CHAR    = 0o040,
        FIFO    = 0o100,
        UNKNOWN = 0o200,
    }
);

#[derive(Debug, Clone)]
pub struct DEntry {
    permission: Permission,
    file_type: FileType,
    name: String
}

#[derive(Debug, Clone, Copy)]
pub struct Cursor(pub usize);

impl Cursor {
    pub fn at_start() -> Self {
        Self(0)
    }
}

pub trait File: Send + Sync + Debug {
    fn write            (&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum>;
    fn read             (&self, length: usize) -> Result<Vec<u8>, ErrorNum>;
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
    fn register_mmap(self: Arc<Self>, mem_layout: &mut MemLayout, offset: usize, length: usize) -> Result<VirtPageNum, ErrorNum>;
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