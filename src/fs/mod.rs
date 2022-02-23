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
};

pub use vfs::{
    VirtualFileSystem,
    Path,
    OpenMode
};

use lazy_static::*;

use crate::utils::Mutex;

lazy_static!{
    pub static ref MOUNT_MANAGER: MountManager = MountManager::new(fs_impl::PARCH_FS.clone());
}

pub fn open(path: &Path, mode: OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
    MOUNT_MANAGER.inner.acquire().open(path, mode)
}