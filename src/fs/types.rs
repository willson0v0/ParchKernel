use alloc::sync::{Arc};
use alloc::vec::Vec;
use crate::mem::{PageGuard};
use crate::utils::ErrorNum;

use super::vfs::OpenMode;
use super::{VirtualFileSystem, Path};

#[derive(Debug, Clone)]
pub struct FileStat {
    pub open_mode   : OpenMode,
    pub file_size   : usize,
    pub path        : Path
}

pub trait File: Send + Sync {
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
    fn stat             (&self) -> FileStat;
}

pub trait SocketFile    : File {}
pub trait LinkFile      : File {}
pub trait RegularFile   : File {
    fn get_page(&self, offset: usize) -> Result<PageGuard, ErrorNum>;
}
pub trait BlockFile     : File {}
pub trait DirFile       : File {
    fn open_dir(&self, rel_path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum>;
}
pub trait CharFile      : File {}
pub trait FIFOFile      : File {}