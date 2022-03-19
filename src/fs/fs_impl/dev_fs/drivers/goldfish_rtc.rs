use core::mem::size_of;

use alloc::sync::Arc;

use crate::{mem::PhysAddr, fs::{File, fs_impl::DEV_FS, VirtualFileSystem, types::FileStat, OpenMode}, utils::ErrorNum};

/// Driver for google goldfish rtc device. Typically mapped at 0x101000
/// 0x00 TIME_LOW
/// 0x04 TIME_HI
/// 0x08 ALARM_LO   // The device will not raise IRQ, these are for compatibility
/// 0x0C ALARM_HI   // The device will not raise IRQ, these are for compatibility
/// 0x10 CLEAR_INT
#[derive(Debug)]
pub struct GoldFishRTC {
    addr: PhysAddr
}

impl GoldFishRTC {
    pub fn new(addr: PhysAddr) -> Self {
        Self {addr}
    }
}

impl File for GoldFishRTC {
    fn write(&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        if length != size_of::<usize>() {
            return Err(ErrorNum::ENOTALIGNED);
        }
        let time_low: u32 = unsafe{(self.addr + 0x00).read_volatile()};
        let time_hi: u32 = unsafe{(self.addr + 0x04).read_volatile()};
        let result: u64 = time_low as u64 + ((time_hi as u64) << 32);
        Ok(result.to_le_bytes().to_vec())
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
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

    fn as_mount<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::MountPoint + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        DEV_FS.clone().as_vfs()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(FileStat{
            open_mode: OpenMode::READ,
            file_size: size_of::<usize>(),
            path: "/dev/rtc0".into(),
            inode: 0,
            fs: Arc::downgrade(&self.vfs()),
        })
    }
}

