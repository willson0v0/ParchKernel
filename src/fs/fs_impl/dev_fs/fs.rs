use crate::{fs::{VirtualFileSystem, Path, File, DirFile, types::{FileStat, Permission}, OpenMode, Dirent, open}, utils::ErrorNum};
use core::fmt::Debug;

use alloc::{sync::Arc, string::ToString};
use lazy_static::*;

use super::UartPTS;

lazy_static!{
    pub static ref DEV_FS: Arc<DevFS> = {
        let res = Arc::new(DevFS());
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

pub struct DevFS();
pub struct DevFolder();

impl Debug for DevFS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DevFS").finish()
    }
}

impl VirtualFileSystem for DevFS {
    fn open(&self, path: &crate::fs::Path, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        DevFolder{}.open_dir(path, mode)
    }

    fn mkdir(&self, _path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn mkfile(&self, _path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn remove(&self, _path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn link(&self, _dest: alloc::sync::Arc<dyn crate::fs::File>, _link_file: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn sym_link(&self, _abs_src: &crate::fs::Path, _rel_dst: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn mount_path(&self) -> Path {
        "/dev".into()
    }

    fn as_vfs<'a>(self: Arc<Self>) -> Arc<dyn VirtualFileSystem + 'a> where Self: 'a {
        self
    }
}

impl Debug for DevFolder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DevFolder").finish()
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

impl DirFile for DevFolder {
    fn open_dir(&self, rel_path: &Path, mode: crate::fs::OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
        if rel_path == &"pts".into() {
            Ok(Arc::new(UartPTS{mode}))
        } else if rel_path == &".".into() || rel_path.is_root() {
            Ok(Arc::new(Self{}))
        } else if rel_path == &"..".into() {
            let mut upper_dir = DEV_FS.mount_path().append("..".to_string()).unwrap();
            upper_dir.reduce();
            open(&upper_dir, mode)
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
        Ok(vec![
            Dirent{ inode: 0, permission: Permission::default(), f_type: crate::fs::types::FileType::DIR, f_name: ".".to_string() },
            Dirent{ inode: 0, permission: Permission::default(), f_type: crate::fs::types::FileType::DIR, f_name: "..".to_string() },
            Dirent{ inode: 0, permission: Permission::default(), f_type: crate::fs::types::FileType::CHAR, f_name: "pts".to_string() },
        ])
    }
}