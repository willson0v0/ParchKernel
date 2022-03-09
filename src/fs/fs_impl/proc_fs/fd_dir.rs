use crate::{process::{ProcessID, FileDescriptor}, fs::{File, DirFile, LinkFile}};


#[derive(Debug)]
pub struct FDDir {
    pub pid: ProcessID,
}

impl File for FDDir {
    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        todo!()
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        todo!()
    }

    fn as_socket<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_link<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_regular   <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_block     <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_dir       <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::DirFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_char      <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_fifo      <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_file      <'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        todo!()
    }

    fn vfs              (&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        todo!()
    }

    fn stat             (&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        todo!()
    }
}

impl DirFile for FDDir {
    fn open_dir(&self, rel_path: &crate::fs::Path, mode: crate::fs::OpenMode) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        todo!()
    }

    fn make_file(&self, name: alloc::string::String, perm: crate::fs::types::Permission, f_type: crate::fs::types::FileType) -> Result<alloc::sync::Arc<dyn File>, crate::utils::ErrorNum> {
        todo!()
    }

    fn remove_file(&self, name: alloc::string::String) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn read_dirent(&self) -> Result<alloc::vec::Vec<crate::fs::Dirent>, crate::utils::ErrorNum> {
        todo!()
    }
}

#[derive(Debug)]
pub struct FDLink {
    pub fd: FileDescriptor
}

impl File for FDLink {
    fn write            (&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        todo!()
    }

    fn read             (&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        todo!()
    }

    fn as_socket    <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::SocketFile   + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_link      <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::LinkFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_regular   <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::RegularFile  + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_block     <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::BlockFile    + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_dir       <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn DirFile      + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_char      <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::CharFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_fifo      <'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::fs::FIFOFile     + 'a>, crate::utils::ErrorNum> where Self: 'a {
        todo!()
    }

    fn as_file      <'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn File + 'a> where Self: 'a {
        todo!()
    }

    fn vfs              (&self) -> alloc::sync::Arc<dyn crate::fs::VirtualFileSystem> {
        todo!()
    }

    fn stat             (&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        todo!()
    }
}

impl LinkFile for FDLink {
    fn read_link(&self) -> Result<crate::fs::Path, crate::utils::ErrorNum> {
        todo!()
    }
}