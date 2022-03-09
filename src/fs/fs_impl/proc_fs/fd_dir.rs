use alloc::{sync::Arc, vec::Vec, string::ToString};

use crate::{process::{ProcessID, FileDescriptor, get_process}, fs::{File, DirFile, LinkFile, types::{FileStat, Permission}, OpenMode, Dirent, VirtualFileSystem}, utils::ErrorNum};

use super::PROC_FS;


#[derive(Debug)]
pub struct FDDir {
    pub pid: ProcessID,
}

impl File for FDDir {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_char<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_mount<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::MountPoint + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        PROC_FS.clone()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(FileStat {
            open_mode: OpenMode::READ,
            file_size: 0,
            path: format!("/proc/{}/fd", self.pid).into(),
            inode: 0,
            fs: Arc::downgrade(&self.vfs()),
        })
    }
}

impl DirFile for FDDir {
    fn open_entry(&self, entry_name: &alloc::string::String, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        let fd: FileDescriptor = entry_name.parse::<usize>().map_err(|_| ErrorNum::ENOENT)?.into();
        let _fd_file_stat = get_process(self.pid)?.get_inner().get_file(fd)?.stat()?;
        Ok(Arc::new(FDLink{
            pid: self.pid,
            fd,
        }))
    }

    fn make_file(&self, name: alloc::string::String, perm: crate::fs::types::Permission, f_type: crate::fs::types::FileType) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn remove_file(&self, name: alloc::string::String) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<crate::fs::Dirent>, crate::utils::ErrorNum> {
        let mut res = Vec::new();

        res.push(Dirent {
            inode: 0,
            permission: Permission::from_bits_truncate(0o755),
            f_type: crate::fs::types::FileType::LINK,
            f_name: ".".to_string(),
        });

        res.push(Dirent {
            inode: 0,
            permission: Permission::from_bits_truncate(0o755),
            f_type: crate::fs::types::FileType::LINK,
            f_name: "..".to_string(),
        });


        let proc = get_process(self.pid)?;
        let proc_inner = proc.get_inner();
        for fd in proc_inner.files.keys() {
            res.push(Dirent {
                inode: 0,
                permission: Permission::from_bits_truncate(0o755),
                f_type: crate::fs::types::FileType::LINK,
                f_name: format!("{}", fd.0),
            });
        }
        
        Ok(res)
    }

    fn register_mount(&self, dentry_name: alloc::string::String, uuid: crate::utils::UUID) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn register_umount(&self, dentry_name: alloc::string::String) -> Result<crate::utils::UUID, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }
}

#[derive(Debug)]
pub struct FDLink {
    pub pid: ProcessID,
    pub fd: FileDescriptor
}

impl File for FDLink {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn DirFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_char<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_mount<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::MountPoint + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        PROC_FS.clone()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(
            FileStat {
                open_mode: OpenMode::READ,
                file_size: 0,
                path: format!("/proc/{}/fd/{}", self.pid.0, self.fd.0).into(),
                inode: 0,
                fs: Arc::downgrade(&PROC_FS.clone().as_vfs()),
            }
        )
    }
}

impl LinkFile for FDLink {
    fn read_link(&self) -> Result<crate::fs::Path, crate::utils::ErrorNum> {
        Ok(get_process(self.pid)?.get_inner().get_file(self.fd)?.stat()?.path)
    }

    fn write_link(&self, path: &crate::fs::Path) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }
}