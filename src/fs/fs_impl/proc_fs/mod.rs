use crate::{fs::VirtualFileSystem, utils::{ErrorNum, UUID}};

mod proc_dir;
mod root_dir;
mod fd_dir;

use lazy_static::*;

use self::root_dir::ROOT_DIR;

lazy_static!{
    pub static ref PROC_FS: alloc::sync::Arc<ProcFS> = alloc::sync::Arc::new(ProcFS{uuid: UUID::new()});
}

#[derive(Debug)]
pub struct ProcFS {
    pub uuid: UUID
}

impl VirtualFileSystem for ProcFS {
    fn link(&self, _dest: alloc::sync::Arc<dyn crate::fs::File>, _link_file: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        Err(ErrorNum::EROFS)
    }

    fn mount_path(&self) -> crate::fs::Path {
        "/proc".into()
    }

    fn as_vfs<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn VirtualFileSystem + 'a> where Self: 'a {
        self
    }

    fn get_uuid(&self) -> crate::utils::UUID {
        self.uuid
    }

    fn root_dir(&self, _mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile>, crate::utils::ErrorNum> {
        Ok(ROOT_DIR.clone())
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync> {
        self
    }
}