use alloc::{sync::{Arc, Weak}, collections::VecDeque, vec::Vec};
use core::fmt::Debug;

use crate::{fs::{File, FIFOFile, types::FileStat, OpenMode, Path}, utils::{SpinMutex, Mutex, ErrorNum}, process::get_processor};

use super::open;

pub struct PipeBuffer {
    pub inner: SpinMutex<PipeBufferInner>
}

pub struct PipeBufferInner {
    pub buffer: VecDeque<u8>
}

impl PipeBufferInner {
    pub fn new() -> Self {
        Self {buffer: VecDeque::new()}
    }
}

impl PipeBuffer {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {inner: SpinMutex::new("pipe", PipeBufferInner::new())})
    }

    pub fn byte_count(&self) -> usize {
        self.inner.acquire().buffer.len()
    }

    // TODO: implement size limit
    pub fn write(&self, data: Vec<u8>) {
        let mut inner = self.inner.acquire();
        inner.buffer.extend(data.iter());
    }

    pub fn read(&self, length: usize) -> Option<Vec<u8>> {
        let mut inner = self.inner.acquire();
        if length <= inner.buffer.len() {
            let new_buf = inner.buffer.split_off(length);
            let res = inner.buffer.clone();
            inner.buffer = new_buf;
            Some(res.into())
        } else {
            None
        }
    }
}

pub struct PipeWriteEnd {
    pub buffer: Arc<PipeBuffer>
}

pub struct PipeReadEnd {
    pub buffer: Weak<PipeBuffer>
}

pub fn new_pipe() -> (Arc<PipeReadEnd>, Arc<PipeWriteEnd>) {
    let buffer = PipeBuffer::new();
    let r = Arc::new(PipeReadEnd{buffer: Arc::downgrade(&buffer)});
    let w = Arc::new(PipeWriteEnd{buffer});
 (r, w)
}

impl Debug for PipeWriteEnd {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Pipe write end, buffer size {}, writer count {}", self.buffer.byte_count(), Arc::strong_count(&self.buffer))
    }
}

impl Debug for PipeReadEnd {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(buf) = self.buffer.upgrade() {
            write!(f, "Pipe reader end, buffer size {}, writer count {}", buf.byte_count(), Arc::strong_count(&buf))
        } else {
            write!(f, "Pipe reader end, pipe broken.")
        }
    }
}

impl File for PipeWriteEnd {
    fn write (&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        let len = data.len();
        self.buffer.write(data);
        Ok(len)
    }

    fn read (&self, _length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn as_socket <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::SocketFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::RegularFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::BlockFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::DirFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_char <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::CharFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::FIFOFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_file <'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn as_any <'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs (&self) -> Arc<dyn crate::fs::VirtualFileSystem> {
        open(&"proc".into(), OpenMode::SYS).unwrap().vfs()
    }

    fn stat (&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(FileStat {
            open_mode: OpenMode::WRITE,
            file_size: self.buffer.byte_count(),
            path: Path::new("[anon pipe]").unwrap(),
            inode: 0,
            fs: Arc::downgrade(&self.vfs()),
        })
    }
}

impl File for PipeReadEnd {
    fn write (&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read (&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        loop {
            if let Some(buf) = self.buffer.upgrade() {
                if let Some(res) = buf.read(length) {
                    return Ok(res);
                } else {
                    get_processor().suspend_switch();
                }
            } else {
                return Err(ErrorNum::EPIPE);
            }
        }
    }

    fn as_socket <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::SocketFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_link <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::LinkFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_regular <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::RegularFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_block <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::BlockFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_dir <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::DirFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_char <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::CharFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Err(ErrorNum::EBADTYPE)
    }

    fn as_fifo <'a>(self: Arc<Self>) -> Result<Arc<dyn crate::fs::FIFOFile + 'a>, crate::utils::ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_file <'a>(self: Arc<Self>) -> Arc<dyn File + 'a> where Self: 'a {
        self
    }

    fn as_any <'a>(self: Arc<Self>) -> Arc<dyn core::any::Any + Send + Sync + 'a> where Self: 'a {
        self
    }

    fn vfs (&self) -> Arc<dyn crate::fs::VirtualFileSystem> {
        open(&"proc".into(), OpenMode::SYS).unwrap().vfs()
    }

    fn stat (&self) -> Result<crate::fs::types::FileStat, crate::utils::ErrorNum> {
        Ok(FileStat {
            open_mode: OpenMode::READ,
            file_size: if let Some(buffer) = self.buffer.upgrade() {buffer.byte_count()} else {0}, 
            path: Path::new("[anon pipe]").unwrap(),
            inode: 0,
            fs: Arc::downgrade(&self.vfs()),
        })
    }
}

impl FIFOFile for PipeWriteEnd {}
impl FIFOFile for PipeReadEnd {}