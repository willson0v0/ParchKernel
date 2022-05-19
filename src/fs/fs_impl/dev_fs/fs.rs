use crate::{config::RTC_ADDR, fs::{VirtualFileSystem, Path, File, DirFile, types::{FileStat, Permission}, OpenMode, Dirent, DummyLink}, utils::{ErrorNum, RWLock, UUID}};
use core::fmt::Debug;

use alloc::{borrow::ToOwned, collections::BTreeMap, string::{ToString, String}, sync::Arc, vec::Vec};
use lazy_static::*;
use crate::device::{DEVICE_MANAGER, Driver};

use super::Adapter;

lazy_static!{
    pub static ref DEV_FS: Arc<DevFS> = {
        let res = Arc::new(DevFS(UUID::new()));
        milestone!("DevFS initialized.");
        res
    };
}

lazy_static!{
    pub static ref DEV_FOLDER: Arc<DevFolder> = {
        let res = Arc::new(DevFolder());
        debug!("DevFolder initialized.");
        res
    };
}

pub struct DevFS(pub UUID);

#[derive(Debug)]
pub struct DevFolder();

impl Debug for DevFS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DevFS").finish()
    }
}

impl VirtualFileSystem for DevFS {
    fn link(&self, _dest: alloc::sync::Arc<dyn crate::fs::File>, _link_file: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn mount_path(&self) -> Path {
        "/dev".into()
    }

    fn as_vfs<'a>(self: Arc<Self>) -> Arc<dyn VirtualFileSystem + 'a> where Self: 'a {
        self
    }

    fn get_uuid(&self) -> crate::utils::UUID {
        self.0.clone()
    }

    fn root_dir(&self, _mode: OpenMode) -> Result<Arc<dyn DirFile>, ErrorNum> {
        Ok(DEV_FOLDER.clone())
    }

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync> {
        self
    }
}

impl File for DevFolder {
    fn write(&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EISDIR)
    }

    fn read(&self, _length: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
        Err(ErrorNum::EISDIR)
    }

    fn as_socket<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::SocketFile + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::LinkFile + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::RegularFile + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::BlockFile + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::DirFile + 'a>, ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_char<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::CharFile + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::FIFOFile + 'a>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> Arc<dyn VirtualFileSystem> {
        DEV_FS.clone()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, ErrorNum> {
        Ok(FileStat{
            open_mode: OpenMode::READ,
            file_size: 0,
            path: "/dev".into(),
            inode: 0,
            fs: Arc::downgrade(&DEV_FS.clone().as_vfs()),
        })
    }
}

impl DevFolder {
    fn compatible_devices() -> Vec<(String, UUID)> {
        let mut res = Vec::new();
        let dev_tree = DEVICE_MANAGER.acquire_r().get_dev_tree();
        let name_list = [
            "google,goldfish-rtc",
            "ns16550a",
            "syscon-poweroff",
            "syscon-reboot",
            "riscv,plic0",
            "virtio,mmio",
        ];
        for comp in name_list {
            let mut driver_list: Vec<(String, UUID)> = dev_tree.serach_compatible(comp).unwrap().iter().map(
                |node| -> (String, UUID) {
                    let node_r = node.acquire_r();
                    (node_r.unit_name.clone(), node_r.driver)
                }
            ).collect();
            res.extend(driver_list);
        }
        res
    }
}

impl DirFile for DevFolder {
    fn open_entry(&self, entry_name: &String, mode: crate::fs::OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
        // TODO: no more hard-coding
        let device_list = Self::compatible_devices();
        let device_map: BTreeMap<String, UUID> = device_list.into_iter().collect();

        if device_map.contains_key(entry_name) {
            Ok(Arc::new(Adapter::new(entry_name, Arc::downgrade(&DEV_FS.clone().as_vfs()), mode)))
        } else if entry_name == "." {
            Ok(Arc::new(DummyLink{
                vfs: DEV_FS.clone(),
                link_dest: "/dev".into(),
                self_path: "/dev/.".into(),
            }))
        } else if entry_name == "pts" {
            Ok(Arc::new(DummyLink{
                vfs: DEV_FS.clone(),
                link_dest: "/dev/uart@10000000".into(),
                self_path: "/dev/pts".into(),
            }))
        } else if entry_name == ".." {
            Ok(Arc::new(DummyLink{
                vfs: DEV_FS.clone(),
                link_dest: "/".into(),
                self_path: "/dev/..".into(),
            }))
        } else {
            Err(ErrorNum::ENOENT)
        }
    }

    fn make_file(&self, _name: alloc::string::String, _perm: crate::fs::types::Permission, _f_type: crate::fs::types::FileType) -> Result<Arc<dyn File>, ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn remove_file(&self, _name: alloc::string::String) -> Result<(), ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<crate::fs::Dirent>, ErrorNum> {
        let device_list = Self::compatible_devices();
        let mut result: Vec<Dirent> = Vec::new();
        for (name, uuid) in device_list.iter() {
            result.push(Dirent {
                inode: uuid.0 as u32,
                permission: Permission::default(),
                f_type: crate::fs::types::FileType::CHAR,
                f_name: name.to_owned(),
            });
        }
        result.push(
            Dirent{ 
                inode: Path::new("/dev/.").unwrap().hash(), 
                permission: Permission::default(), 
                f_type: crate::fs::types::FileType::LINK, 
                f_name: ".".to_string() }
        );
        result.push(
            Dirent{ 
                inode: Path::new("/dev/..").unwrap().hash(), 
                permission: Permission::default(), 
                f_type: crate::fs::types::FileType::LINK, 
                f_name: "..".to_string() }
        );
        result.push(
            Dirent{ 
                inode: Path::new("/dev/pts").unwrap().hash(), 
                permission: Permission::default(), 
                f_type: crate::fs::types::FileType::LINK, 
                f_name: "pts".to_string() }
        );

        Ok(result)
    }
}