use alloc::{sync::Arc, borrow::ToOwned, vec::Vec, string::ToString};

use crate::{fs::{File, DirFile, LinkFile, types::{FileStat, Permission}, OpenMode, Path, VirtualFileSystem, Dirent}, process::{ProcessID, get_process, get_processor}, utils::ErrorNum};

use super::{PROC_FS, fd_dir::FDDir};

#[derive(Debug)]
pub struct SelfProcDir;

impl File for SelfProcDir {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile   + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_regular<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile  + 'a>, crate::utils::ErrorNum> where Self: 'a {
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

    fn as_file<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn vfs(&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        PROC_FS.clone()
    }

    fn stat(&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(FileStat {
            open_mode: OpenMode::READ,
            file_size: 0,
            path: Path::new("/proc/self").unwrap(),
            inode: 0,
            fs: Arc::downgrade(&PROC_FS.clone().as_vfs()),
        })
    }
}

impl LinkFile for SelfProcDir {
    fn read_link(&self) -> Result<crate::fs::Path, crate::utils::ErrorNum> {
        Ok(format!("/proc/{}", get_processor().current().unwrap().pid.0).into())
    }
}

#[derive(Debug)]
pub struct PidProcDir{
    pub pid: ProcessID
}

impl File for PidProcDir {
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
        Ok(FileStat {
            open_mode: OpenMode::READ,
            file_size: 0,
            path: Path::new_s(format!("/proc/{}", self.pid.0)).unwrap(),
            inode: 0,
            fs: Arc::downgrade(&PROC_FS.clone().as_vfs()),
        })
    }
}

impl DirFile for PidProcDir {
    fn open_dir(&self, rel_path: &crate::fs::Path, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        if rel_path.to_owned() == Path::new("fd").unwrap() {
            Ok(Arc::new(FDDir{
                pid: self.pid,
            }))
        } else if rel_path.starts_with(&"..".into()) {
            Ok(Arc::new(ProcDir{}))
        } else if rel_path.starts_with(&".".into()) {
            Ok(Arc::new(Self{pid: self.pid}))
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
        let mut res = Vec::new();

        res.push(Dirent{
            inode: 0,
            permission: Permission::default(),
            f_type: crate::fs::types::FileType::LINK,
            f_name: ".".to_string(),
        });

        res.push(Dirent{
            inode: 0,
            permission: Permission::default(),
            f_type: crate::fs::types::FileType::LINK,
            f_name: "..".to_string(),
        });

        let proc = get_process(self.pid)?;
        let proc_inner = proc.get_inner();
        
        for (fd, file) in proc_inner.files.iter() {
            // let file_stat = file.stat().unwrap();
            res.push(Dirent{
                inode: 0,
                permission: Permission::default(),
                f_type: crate::fs::types::FileType::LINK,
                f_name: format!("{}", fd.0),
            })
        }

        Ok(res)
    }
}

impl PidProcDir {
    fn new(&self, pid: ProcessID) -> Result<Self, ErrorNum> {
        let _proc = get_process(pid)?; // check process exist
        Ok(Self{pid})
    }
}


#[derive(Debug)]
pub struct ProcDir;

impl File for ProcDir {
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
        Ok(FileStat {
            open_mode: OpenMode::READ,
            file_size: 0,
            path: Path::new("/proc").unwrap(),
            inode: 0,
            fs: Arc::downgrade(&PROC_FS.clone().as_vfs()),
        })
    }
}

impl DirFile for ProcDir {
    fn open_dir(&self, rel_path: &crate::fs::Path, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        let pid_str = rel_path.components[0].clone();
        let pid = usize::from_str_radix(pid_str.as_str(), 10).map_err(|_| ErrorNum::ENOENT)?;
        let pid = ProcessID(pid);
        Ok(Arc::new(PidProcDir{pid}))
    }

    fn make_file(&self, name: alloc::string::String, perm: crate::fs::types::Permission, f_type: crate::fs::types::FileType) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn remove_file(&self, name: alloc::string::String) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<crate::fs::Dirent>, crate::utils::ErrorNum> {
        // let mut res = Vec::new();

        // let proc = get_process(self.pid)?;
        // let proc_inner = proc.get_inner();
        
        // for (fd, file) in proc_inner.files.iter() {
        //     // let file_stat = file.stat().unwrap();
        //     res.push(Dirent{
        //         inode: 0,
        //         permission: Permission::default(),
        //         f_type: crate::fs::types::FileType::LINK,
        //         f_name: format!("{}", fd.0),
        //     })
        // }

        // Ok(res)
        todo!()
    }
}