mod manager;
mod types;
mod fs_impl;
mod vfs;
mod pipes;

// pub use mount_point::MountPoint;

use alloc::sync::Arc;
pub use manager::{
    MountManager
};

pub use types::{
    File        ,
    SocketFile  ,
    LinkFile    ,
    RegularFile ,
    BlockFile   ,
    DirFile     ,
    CharFile    ,
    FIFOFile    ,
    DummyLink   ,
    Cursor      ,
    Dirent      ,
    FileType    ,
    Permission
};

pub use vfs::{
    VirtualFileSystem,
    Path,
    OpenMode
};

pub use pipes::{
    PipeReadEnd,
    PipeWriteEnd,
    new_pipe
};

use lazy_static::*;

use crate::utils::{RWLock, ErrorNum};

lazy_static!{
    pub static ref MOUNT_MANAGER: MountManager = {
        let root_fs = fs_impl::PARCH_FS.clone();
        let res = MountManager::new(root_fs);
        verbose!("Mount manager initialized");
        res
    };
}

pub fn open(path: &Path, mode: OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
    MOUNT_MANAGER.inner.acquire_r().open(path, mode)
}

pub fn open_at(file: Arc<dyn File>, rel_path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
    MOUNT_MANAGER.inner.acquire_r().open_at(file, rel_path, mode)
}

pub fn delete(path: &Path) -> Result<(), ErrorNum> {
    MOUNT_MANAGER.inner.acquire_r().remove(path)
}

pub fn make_file(path: &Path, permission: Permission, f_type: FileType) -> Result<(), ErrorNum> {
    MOUNT_MANAGER.inner.acquire_r().make_file(path, permission, f_type)
}

pub fn make_file_at(path: &Path, root: Arc<dyn File>, permission: Permission, f_type: FileType) -> Result<(), ErrorNum> {
    MOUNT_MANAGER.inner.acquire_r().make_file_at(path, root, permission, f_type)
}

pub fn init() {
    verbose!("Initializing /dev mount point");
    MOUNT_MANAGER.inner.acquire_r().make_file(&"/dev".into(), Permission::from_bits_truncate(0o544), types::FileType::DIR).expect("Failed to create dev fs mount point.");
    verbose!("Initializing /dev");
    MOUNT_MANAGER.inner.acquire_w().mount("/dev".into(), fs_impl::DEV_FS.clone()).expect("Failed to mount dev fs.");
    verbose!("Initializing /proc mount point");
    MOUNT_MANAGER.inner.acquire_r().make_file(&"/proc".into(), Permission::from_bits_truncate(0o544), types::FileType::DIR).expect("Failed to create proc fs mount point.");
    verbose!("Initializing /proc");
    MOUNT_MANAGER.inner.acquire_w().mount("/proc".into(), fs_impl::PROC_FS.clone()).expect("Failed to mount proc fs.");
}