use alloc::{sync::Arc, vec::Vec, string::ToString};

use crate::{fs::{File, DirFile, types::{FileStat, Permission}, OpenMode, VirtualFileSystem, fs_impl::proc_fs::{proc_dir::{PidProcDir, SelfProcDir}}, Dirent, DummyLink}, utils::ErrorNum, process::{ProcessID, get_process, process_list}};

use super::{PROC_FS};

use lazy_static::*;

lazy_static!{
    pub static ref ROOT_DIR: Arc<RootDir> = Arc::new(RootDir{});
}

#[derive(Debug)]
pub struct RootDir;

impl File for RootDir {
    fn write(&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EISDIR)
    }

    fn read(&self, _length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
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
    
    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
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
    fn open_entry(&self, entry_name: &alloc::string::String, _mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
        if entry_name == "self" {
            Ok(Arc::new(SelfProcDir{}))
        } else if entry_name == ".." {
            Ok(Arc::new(DummyLink{
                vfs: PROC_FS.clone(),
                link_dest: "/".into(),
                self_path: "/proc/..".into(),
            }))
        } else if entry_name == "." {
            Ok(Arc::new(DummyLink{
                vfs: PROC_FS.clone(),
                link_dest: "/proc".into(),
                self_path: "/proc/.".into(),
            }))
        } else {
            let pid: ProcessID = entry_name.parse::<usize>().map_err(|_| ErrorNum::ENOENT)?.into();
            let _proc = get_process(pid)?;  // make sure there is such process.
            Ok(Arc::new(PidProcDir { pid }))
        }
    }

    fn make_file(&self, _name: alloc::string::String, _perm: crate::fs::types::Permission, _f_type: crate::fs::types::FileType) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn remove_file(&self, _name: alloc::string::String) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<crate::fs::Dirent>, crate::utils::ErrorNum> {
        let mut result = Vec::new();

        result.push(Dirent {
            inode: 0,
            permission: Permission::from_bits_truncate(0o440),
            f_type: crate::fs::types::FileType::LINK,
            f_name: ".".to_string(),
        });

        result.push(Dirent {
            inode: 0,
            permission: Permission::from_bits_truncate(0o440),
            f_type: crate::fs::types::FileType::LINK,
            f_name: "..".to_string(),
        });

        result.push(Dirent {
            inode: 0,
            permission: Permission::from_bits_truncate(0o440),
            f_type: crate::fs::types::FileType::LINK,
            f_name: "self".to_string(),
        });

        let process_list = process_list();
        for pcb in process_list {
            let dentry = Dirent {
                inode: 0,
                permission: Permission::from_bits_truncate(0o440),
                f_type: crate::fs::types::FileType::DIR,
                f_name: format!("{}", pcb.pid.0),
            };
            result.push(dentry);
        }
        Ok(result)
    }
}