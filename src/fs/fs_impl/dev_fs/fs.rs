use crate::{fs::{VirtualFileSystem, Path}, utils::ErrorNum};
use core::fmt::Debug;

use alloc::{sync::Arc, borrow::ToOwned};
use lazy_static::*;

use super::UartPTS;

lazy_static!{
    pub static ref DEV_FS: Arc<DevFS> = Arc::new(DevFS());
}

pub struct DevFS();

impl Debug for DevFS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("DevFS").finish()
    }
}

impl VirtualFileSystem for DevFS {
    fn open(&self, path: &crate::fs::Path, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        if path.to_owned() == Path::from("/pts") {
            Ok(Arc::new(UartPTS{mode}))
        } else {
            Err(ErrorNum::ENOENT)
        }
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
}