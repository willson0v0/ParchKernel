use alloc::{sync::Arc, collections::BTreeMap};

use crate::{mem::{MemLayout, VirtAddr}, utils::{SpinMutex, MutexGuard, Mutex, ErrorNum}, fs::{Path, open, OpenMode, RegularFile, File}};

use super::{ProcessID, new_pid, processor::ProcessContext};

#[derive(PartialEq, Eq)]
pub enum ProcessStatus {
    Initialized,
    Ready,
    Running,
    Zombie
}

pub struct ProcessControlBlock {
    pub pid: ProcessID,
    pub elf_file: Arc<dyn RegularFile>,
    inner: SpinMutex<PCBInner>
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileDescriptor(usize);

impl From<usize> for FileDescriptor {
    fn from(c: usize) -> Self {
        Self(c)
    }
}

pub struct PCBInner {
    pub mem_layout: MemLayout,
    pub status: ProcessStatus,
    pub proc_context: ProcessContext,
    pub entry_point: VirtAddr,
    pub fd: BTreeMap<FileDescriptor, Arc<dyn File>>,
    pub trace: bool
}

impl ProcessControlBlock {
    pub fn new(elf_path: Path) -> Result<Arc<Self>, ErrorNum> {
        verbose!("Initializing PCB for {:?}", elf_path);
        let elf_file = open(&elf_path, OpenMode::SYS)?.as_regular()?;
        let mut mem_layout = MemLayout::new();
        mem_layout.map_proc_stack();
        let pid = new_pid();
        let res = Arc::new(Self {
            pid,
            elf_file,
            inner: SpinMutex::new("pcb lock", PCBInner::new(mem_layout))
        });
        verbose!("PCB for {:?} Initialized", elf_path);
        Ok(res)
    }

    pub fn get_inner(&self) -> MutexGuard<PCBInner> {
        self.inner.acquire()
    }
}

impl PCBInner {
    pub fn new(mem_layout: MemLayout) -> Self {
        let mut fd: BTreeMap<FileDescriptor, Arc<dyn File>> = BTreeMap::new();
        fd.insert(0.into(), open(&Path::new("/dev/pts").unwrap(), OpenMode::READ).unwrap());
        fd.insert(1.into(), open(&Path::new("/dev/pts").unwrap(), OpenMode::WRITE).unwrap());
        fd.insert(2.into(), open(&Path::new("/dev/pts").unwrap(), OpenMode::WRITE).unwrap());
        Self {
            mem_layout,
            status: ProcessStatus::Initialized,
            entry_point: 0.into(),
            proc_context: ProcessContext::new(),
            fd,
            trace: if cfg!(debug_assertions) {true} else  {false}
        }
    }

    pub fn context_ptr(&mut self) -> *mut ProcessContext {
        // use identical mapping.
        &mut self.proc_context
    }
}