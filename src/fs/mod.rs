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
    MountPoint  ,
    DummyLink   ,
    Cursor,
    Dirent
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

use self::types::Permission;

lazy_static!{
    pub static ref MOUNT_MANAGER: MountManager = MountManager::new(fs_impl::PARCH_FS.clone());
}

pub fn open(path: &Path, mode: OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
    MOUNT_MANAGER.inner.acquire_r().open(path, mode)
}

pub fn open_at(file: Arc<dyn File>, rel_path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
    MOUNT_MANAGER.inner.acquire_r().open_at(file, rel_path, mode)
}

pub fn init() {
    MOUNT_MANAGER.inner.acquire_r().make_file(&"/dev".into(), Permission::from_bits_truncate(0o544), types::FileType::DIR).expect("Failed to create dev fs mount point.");
    MOUNT_MANAGER.inner.acquire_w().mount("/dev".into(), fs_impl::DEV_FS.clone()).expect("Failed to mount dev fs.");
    MOUNT_MANAGER.inner.acquire_r().make_file(&"/proc".into(), Permission::from_bits_truncate(0o544), types::FileType::DIR).expect("Failed to create proc fs mount point.");
    MOUNT_MANAGER.inner.acquire_w().mount("/proc".into(), fs_impl::PROC_FS.clone()).expect("Failed to mount proc fs.");
}