use alloc::sync::Arc;

use crate::{fs::{File, DirFile, types::FileStat, OpenMode, VirtualFileSystem, fs_impl::proc_fs::{fd_dir::FDDir, proc_dir::{PidProcDir, ProcDir}}, open, Path}, utils::ErrorNum};

use super::{PROC_FS, proc_dotlink::ProcDotDir};

#[derive(Debug)]
pub struct RootDir;

impl File for RootDir {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EISDIR)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        Err(ErrorNum::EISDIR)
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile   + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile  + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn DirFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_char<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        PROC_FS.clone()
    }
    
    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(FileStat{
            open_mode: OpenMode::READ,
            file_size: 0,
            path: "/proc".into(),
            inode: 0,
            fs: Arc::downgrade(&PROC_FS.clone().as_vfs()),
        })
    }
}

impl DirFile for RootDir {
    fn open_dir(&self, rel_path: &crate::fs::Path, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        if rel_path.starts_with(&"proc".into()) {
            ProcDir{}.open_dir(&rel_path.strip_head(), mode)
        } else if rel_path.starts_with(&"..".into())  {
            Ok(Arc::new(ProcDotDir::new("proc/..".into(), rel_path.strip_head())))
        } else if rel_path.starts_with(&".".into()) {
            Ok(Arc::new(ProcDotDir::new("proc/.".into(), Path::new("/proc").unwrap().concat(&rel_path.strip_head()))))
        } else {
            Err(ErrorNum::ENOENT)
        }
    }

    fn make_file(&self, name: alloc::string::String, perm: crate::fs::types::Permission, f_type: crate::fs::types::FileType) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn remove_file(&self, name: alloc::string::String) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<crate::fs::Dirent>, crate::utils::ErrorNum> {
        todo!()
    }
}