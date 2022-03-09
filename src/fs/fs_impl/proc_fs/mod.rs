use crate::fs::VirtualFileSystem;

mod pipes;
mod proc_dir;
mod root_dir;
mod fd_dir;
mod proc_dotlink;

use lazy_static::*;

lazy_static!{
    pub static ref PROC_FS: alloc::sync::Arc<ProcFS> = alloc::sync::Arc::new(ProcFS{});
}

#[derive(Debug)]
pub struct ProcFS {}

impl VirtualFileSystem for ProcFS {
    fn open(&self, path: &crate::fs::Path, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        todo!()
    }

    fn mkdir(&self, path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn mkfile(&self, path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn remove(&self, path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn link(&self, dest: alloc::sync::Arc<dyn crate::fs::File>, link_file: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::File>, crate::utils::ErrorNum> {
        todo!()
    }

    fn sym_link(&self, abs_src: &crate::fs::Path, rel_dst: &crate::fs::Path) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile>, crate::utils::ErrorNum> {
        todo!()
    }

    fn mount_path(&self) -> crate::fs::Path {
        todo!()
    }

    fn as_vfs<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn VirtualFileSystem + 'a> where Self: 'a {
        todo!()
    }
}