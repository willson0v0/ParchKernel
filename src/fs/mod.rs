mod manager;
mod types;
mod fs_impl;
mod vfs;

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
    Cursor,
};

pub use vfs::{
    VirtualFileSystem,
    Path,
    OpenMode
};

use lazy_static::*;

use crate::utils::Mutex;

lazy_static!{
    pub static ref MOUNT_MANAGER: MountManager = MountManager::new();
}

pub fn open(path: &Path, mode: OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
    MOUNT_MANAGER.inner.acquire().open(path, mode)
}

pub fn init() {
    let mut inner = MOUNT_MANAGER.inner.acquire();
    inner.mount("/".into(), fs_impl::PARCH_FS.clone()).expect("Failed to mount root fs");
    inner.mount("/dev".into(), fs_impl::DEV_FS.clone()).expect("Failed to mount dev fs");
}