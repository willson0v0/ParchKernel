use core::fmt::Debug;

use crate::{fs::{File, types::FileStat, OpenMode, CharFile, VirtualFileSystem}, utils::{UART0, ErrorNum}};

use alloc::sync::Arc;


use super::DEV_FS;

pub struct UartPTS{
    pub mode: OpenMode
}

impl Drop for UartPTS {
    fn drop(&mut self) {
        // Nothing
    }
}

impl Debug for UartPTS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Uart PTS")
    }
}

impl File for UartPTS {
    fn write(&self, data: alloc::vec::Vec::<u8>, _offset: usize) -> Result<(), crate::utils::ErrorNum> {
        UART0.write_data(&data);
        Ok(())
    }

    fn read(&self, length: usize, offset: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        if offset != 0 {
            Err(ErrorNum::EOOR)
        } else {
            Ok(UART0.read_bytes(length))
        }
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile   + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile  + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile    + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile      + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_char<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_fifo<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        DEV_FS.clone()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        let fs: Arc<dyn VirtualFileSystem> = DEV_FS.clone();
        let fs = Arc::downgrade(&fs);
        Ok(
            FileStat{
                open_mode: self.mode,
                file_size: 0,
                path: "/dev/pts0".into(),
                inode: 0,
                fs
            }
        )
    }
}

impl CharFile for UartPTS {}