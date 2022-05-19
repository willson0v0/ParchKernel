use alloc::sync::{Arc, Weak};
use core::fmt::Debug;
use crate::{device::{DTBNode, Driver}, fs::{CharFile, File, VirtualFileSystem, types::FileStat}, utils::{RWLock, SpinRWLock}};
use crate::utils::ErrorNum;
use crate::fs::OpenMode;
use crate::device::DEVICE_MANAGER;

pub struct Adapter {
    driver: Arc<dyn Driver>,
    dev_node: Arc<SpinRWLock<DTBNode>>,
    fs: Weak<dyn VirtualFileSystem>,
    open_mode: OpenMode,
}

impl Debug for Adapter {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Devfs driver Adapter for {}", self.dev_node.acquire_r().unit_name)
    }
}

impl Adapter {
    pub fn new(unit_name: &str, fs: Weak<dyn VirtualFileSystem>, open_mode: OpenMode) -> Self {
        debug!("Creating fs adapter for {}", unit_name);
        let device_mgr = DEVICE_MANAGER.acquire_r();
        let dev_tree = device_mgr.get_dev_tree();
        let dev_node = dev_tree.search_name(unit_name).unwrap();
        let driver = device_mgr.get_device(dev_node.acquire_r().driver).unwrap();
        Self {
            driver,
            dev_node,
            fs,
            open_mode,
        }
    }
}

impl File for Adapter {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        self.driver.write(data)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        self.driver.read(length)
    }

    fn as_socket<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::SocketFile   + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::LinkFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::RegularFile  + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::BlockFile    + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::DirFile      + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_char<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::CharFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_fifo<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::FIFOFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> Arc<dyn crate::fs::VirtualFileSystem> {
        self.fs.upgrade().unwrap()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        let dev_node = self.dev_node.acquire_r();
        Ok(FileStat{
            open_mode: self.open_mode,
            file_size: 0,
            path: format!("/dev/{}", dev_node.unit_name).into(),
            inode: dev_node.driver.0 as u32,   // use driver lower 32-bit
            fs: self.fs.clone(),
        })
    }
}

impl CharFile for Adapter {
    fn ioctl(&self, op: usize, data: alloc::boxed::Box<dyn core::any::Any>) -> Result<alloc::boxed::Box<dyn core::any::Any>, ErrorNum> {
        self.driver.ioctl(op, data)
    }
}