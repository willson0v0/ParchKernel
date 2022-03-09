use alloc::sync::Arc;

use crate::{fs::{Path, File, DirFile, LinkFile, types::FileStat, OpenMode, VirtualFileSystem}, utils::ErrorNum};

use super::PROC_FS;

#[derive(Debug)]
pub struct ProcDotDir {
    pub path: Path,
    pub link: Path
}

impl File for ProcDotDir {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_char<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        PROC_FS.clone()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(FileStat{
            open_mode: OpenMode::NO_FOLLOW | OpenMode::READ,
            file_size: 0,
            path: self.path.clone(),
            inode: 0,
            fs: Arc::downgrade(&PROC_FS.clone().as_vfs()),
        })
    }
}

impl LinkFile for ProcDotDir {
    fn read_link(&self) -> Result<Path, crate::utils::ErrorNum> {
        Ok(self.link.clone())
    }
}

impl ProcDotDir {
    pub fn new(path: Path, link: Path) -> Self {
        Self{path, link}
    }
}
