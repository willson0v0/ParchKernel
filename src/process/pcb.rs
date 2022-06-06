use core::{mem::size_of, cmp::Ordering};

use alloc::{collections::{BTreeMap, LinkedList, VecDeque}, sync::{Arc, Weak}, vec::Vec};

use crate::{mem::{MemLayout, VirtAddr, VirtPageNum}, utils::{SpinMutex, MutexGuard, Mutex, ErrorNum}, fs::{Path, open, OpenMode, RegularFile, File}, interrupt::trap_context::TrapContext, config::{TRAP_CONTEXT_ADDR, PROC_U_STACK_ADDR, PROC_U_STACK_SIZE, U_TRAMPOLINE_ADDR, MAX_FD, MAX_SYSCALL}, process::{def_handler::*, get_processor}, syscall::syscall_num::{SYSCALL_WRITE, SYSCALL_READ}};

use super::{ProcessID, new_pid, processor::ProcessContext, SignalNum};

#[derive(PartialEq, Eq)]
pub enum ProcessStatus {
    Init,
    Ready,
    Running,
    Zombie
}

pub struct ProcessControlBlock {
    pub pid: ProcessID,
    pub inner: SpinMutex<PCBInner>
}

impl Eq for ProcessControlBlock {}

impl PartialEq for ProcessControlBlock {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid
    }
}

impl PartialOrd for ProcessControlBlock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}


impl Ord for ProcessControlBlock {
    fn cmp(&self, other: &Self) -> Ordering {
        self.pid.cmp(&other.pid)
    }
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
pub struct FileDescriptor(pub usize);

impl From<usize> for FileDescriptor {
    fn from(c: usize) -> Self {
        Self(c)
    }
}

pub struct PCBInner {
    pub elf_file: Arc<dyn RegularFile>,
    pub mem_layout: MemLayout,
    pub status: ProcessStatus,
    pub proc_context: ProcessContext,
    pub entry_point: VirtAddr,
    pub data_end: VirtAddr,
    pub files: BTreeMap<FileDescriptor, Arc<dyn File>>,
    pub signal_handler: BTreeMap<SignalNum, VirtAddr>,
    pub pending_signal: VecDeque<SignalNum>,
    pub signal_contexts: Vec<TrapContext>,
    pub signal_enable: BTreeMap<SignalNum, bool>,
    pub children: LinkedList<Arc<ProcessControlBlock>>,
    pub parent: Option<Weak<ProcessControlBlock>>,
    pub exit_code: Option<isize>,
    pub cwd: Path,
    pub trace_enabled: [bool; MAX_SYSCALL]
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
            inner: SpinMutex::new("pcb lock", PCBInner::new(mem_layout, elf_file))
        });
        verbose!("PCB for {:?} Initialized", elf_path);
        Ok(res)
    }

    pub fn get_inner(&self) -> MutexGuard<PCBInner> {
        self.inner.acquire()
    }

    pub fn fork(self: &Arc<Self>) -> Result<Arc<Self>, ErrorNum> {
        Ok(Arc::new(Self {
            pid: new_pid(),
            inner: SpinMutex::new("pcb lock", self.get_inner().fork(Arc::downgrade(self))?)
        }))
    }
}

impl Drop for ProcessControlBlock {
    fn drop(&mut self) {
        warning!("{:?} was freed.", self.pid);
    }
}

impl PCBInner {
    pub fn default_fds() -> Result<BTreeMap<FileDescriptor, Arc<dyn File>>, ErrorNum> {
        let files: BTreeMap<FileDescriptor, Arc<dyn File>> = BTreeMap::new();
        // files.insert(0.into(), open(&Path::new("/dev/pts")?, OpenMode::READ )?);
        // files.insert(1.into(), open(&Path::new("/dev/pts")?, OpenMode::WRITE)?);
        // files.insert(2.into(), open(&Path::new("/dev/pts")?, OpenMode::WRITE)?);
        Ok(files)
    }

    fn default_trace() -> [bool; MAX_SYSCALL] {
        if cfg!(debug_assertions) {
            let mut res = [true; MAX_SYSCALL];
            res[SYSCALL_WRITE] = false;
            res[SYSCALL_READ] = false;
            res
        } else {
            [false; MAX_SYSCALL]
        }
    }

    pub fn new(mem_layout: MemLayout, elf_file: Arc<dyn RegularFile>) -> Self {
        let signal_handler = Self::default_hander();
        let signal_enable = Self::defualt_mask();

        Self {
            elf_file,
            mem_layout,
            status: ProcessStatus::Init,
            entry_point: 0.into(),
            data_end: 0.into(),
            proc_context: ProcessContext::new(),
            files: Self::default_fds().unwrap(),
            trace_enabled: Self::default_trace(),
            signal_handler,
            signal_contexts: Vec::new(),
            signal_enable,
            children: LinkedList::new(),
            parent: None,
            exit_code: None,
            cwd: Path::root(),
            pending_signal: VecDeque::new(),
        }
    }

    pub fn default_hander() -> BTreeMap<SignalNum, VirtAddr> {
        extern "C" {fn strampoline(); fn sutrampoline(); }
        
        let terminate_self_va   = U_TRAMPOLINE_ADDR + (def_terminate_self as usize - sutrampoline as usize);
        let ignore_va           = U_TRAMPOLINE_ADDR + (def_ignore         as usize - sutrampoline as usize);
        let dump_core_va        = U_TRAMPOLINE_ADDR + (def_dump_core      as usize - sutrampoline as usize);
        let cont_va             = U_TRAMPOLINE_ADDR + (def_cont           as usize - sutrampoline as usize);
        let stop_va             = U_TRAMPOLINE_ADDR + (def_stop           as usize - sutrampoline as usize);

        let mut signal_handler = BTreeMap::new();
        signal_handler.insert(SignalNum::SIGHUP   , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGINT   , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGQUIT  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGILL   , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGTRAP  , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGABRT  , dump_core_va     .clone());
        signal_handler.insert(SignalNum::SIGBUS   , dump_core_va     .clone());
        signal_handler.insert(SignalNum::SIGFPE   , dump_core_va     .clone());
        signal_handler.insert(SignalNum::SIGKILL  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGUSR1  , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGSEGV  , dump_core_va     .clone());
        signal_handler.insert(SignalNum::SIGUSR2  , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGPIPE  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGALRM  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGTERM  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGSTKFLT, terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGCHLD  , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGCONT  , cont_va          .clone());
        signal_handler.insert(SignalNum::SIGSTOP  , stop_va          .clone());
        signal_handler.insert(SignalNum::SIGTSTP  , stop_va          .clone());
        signal_handler.insert(SignalNum::SIGTTIN  , stop_va          .clone());
        signal_handler.insert(SignalNum::SIGTTOU  , stop_va          .clone());
        signal_handler.insert(SignalNum::SIGURG   , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGXCPU  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGXFSZ  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGVTALRM, ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGPROF  , terminate_self_va.clone());
        signal_handler.insert(SignalNum::SIGWINCH , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGIO    , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGPWR   , ignore_va        .clone());
        signal_handler.insert(SignalNum::SIGSYS   , terminate_self_va.clone());
        signal_handler
    }

    pub fn defualt_mask() -> BTreeMap<SignalNum, bool> {
        let mut signal_mask = BTreeMap::new();
        signal_mask.insert(SignalNum::SIGHUP   , true);
        signal_mask.insert(SignalNum::SIGINT   , true);
        signal_mask.insert(SignalNum::SIGQUIT  , true);
        signal_mask.insert(SignalNum::SIGILL   , true);
        signal_mask.insert(SignalNum::SIGTRAP  , true);
        signal_mask.insert(SignalNum::SIGABRT  , true);
        signal_mask.insert(SignalNum::SIGBUS   , true);
        signal_mask.insert(SignalNum::SIGFPE   , true);
        signal_mask.insert(SignalNum::SIGKILL  , true);
        signal_mask.insert(SignalNum::SIGUSR1  , true);
        signal_mask.insert(SignalNum::SIGSEGV  , true);
        signal_mask.insert(SignalNum::SIGUSR2  , true);
        signal_mask.insert(SignalNum::SIGPIPE  , true);
        signal_mask.insert(SignalNum::SIGALRM  , true);
        signal_mask.insert(SignalNum::SIGTERM  , true);
        signal_mask.insert(SignalNum::SIGSTKFLT, true);
        signal_mask.insert(SignalNum::SIGCHLD  , true);
        signal_mask.insert(SignalNum::SIGCONT  , true);
        signal_mask.insert(SignalNum::SIGSTOP  , true);
        signal_mask.insert(SignalNum::SIGTSTP  , true);
        signal_mask.insert(SignalNum::SIGTTIN  , true);
        signal_mask.insert(SignalNum::SIGTTOU  , true);
        signal_mask.insert(SignalNum::SIGURG   , true);
        signal_mask.insert(SignalNum::SIGXCPU  , true);
        signal_mask.insert(SignalNum::SIGXFSZ  , true);
        signal_mask.insert(SignalNum::SIGVTALRM, true);
        signal_mask.insert(SignalNum::SIGPROF  , true);
        signal_mask.insert(SignalNum::SIGWINCH , true);
        signal_mask.insert(SignalNum::SIGIO    , true);
        signal_mask.insert(SignalNum::SIGPWR   , true);
        signal_mask.insert(SignalNum::SIGSYS   , true);
        signal_mask
    }
    
    pub fn get_context(&mut self) -> *mut ProcessContext {
        (&mut self.proc_context) as *mut ProcessContext
    }

    pub fn fork(&mut self, parent: Weak<ProcessControlBlock>) -> Result<Self, ErrorNum> {
        Ok(Self {
            elf_file: self.elf_file.clone(),
            mem_layout: self.mem_layout.fork()?,
            status: ProcessStatus::Ready,
            proc_context: ProcessContext::new(),
            entry_point: self.entry_point,
            data_end: self.data_end,
            files: self.files.clone(),
            trace_enabled: self.trace_enabled.clone(),
            signal_contexts: Vec::new(),
            signal_handler: self.signal_handler.clone(),    // save signal handler
            signal_enable: self.signal_enable.clone(),
            children: LinkedList::new(),
            parent: Some(parent),
            exit_code: None,
            cwd: self.cwd.clone(),
            pending_signal: VecDeque::new(),    // clear pending signal
        })
    }

    pub fn trap_context(&self) -> &'static mut TrapContext {
        let vpn: VirtPageNum = TRAP_CONTEXT_ADDR.into();
        let ppn = self.mem_layout.pagetable.translate(vpn).unwrap();
        unsafe{TrapContext::from_pa(ppn.into())}
    }

    pub fn recv_signal(&mut self, signal: SignalNum) -> Result<(), ErrorNum> {
        if !self.signal_enable.get(&signal).unwrap_or(&false) {
            return Err(ErrorNum::ESIGDISABLED);
        }
        self.pending_signal.push_back(signal);
        Ok(())
    }

    pub fn get_file(&self, fd: FileDescriptor) -> Result<Arc<dyn File>, ErrorNum> {
        self.files.get(&fd).ok_or(ErrorNum::EBADFD).cloned()
    }

    pub fn register_file(&mut self, file: Arc<dyn File>) -> Result<FileDescriptor, ErrorNum> {
        if self.files.len() > MAX_FD {
            return Err(ErrorNum::EMFILE)
        }
        let mut fd = FileDescriptor::from(0);
        loop {
            if self.files.contains_key(&fd) {
                fd.0 += 1;
            } else {
                break;
            }
        }
        self.files.insert(fd, file);
        Ok(fd)
    }

    pub fn close_file(&mut self, fd: FileDescriptor) -> Result<(), ErrorNum> {
        self.files.remove(&fd).map(|_| ()).ok_or(ErrorNum::EBADFD)
    }

    pub fn dup_file(&mut self, to_dup: FileDescriptor) -> Result<FileDescriptor, ErrorNum> {
        let to_dup = self.get_file(to_dup)?;
        self.register_file(to_dup)
    }

    pub fn exec(&mut self, elf_file: Arc<dyn RegularFile>, args: Vec<Vec<u8>>) -> Result<(), ErrorNum> {
        assert!(self.status == ProcessStatus::Running, "Exec on process that is not running");
        self.mem_layout.reset()?;
        self.elf_file = elf_file.clone();
        let (entry, data) = self.mem_layout.map_elf(elf_file.clone())?;
        self.mem_layout.do_map();
        verbose!("mem_layout done");
        self.entry_point = entry;
        self.data_end = data;
        // preserve file descriptor table
        // self.files = Self::default_fds()?;
        self.trace_enabled = Self::default_trace();
        self.signal_contexts.clear();
        self.signal_handler = Self::default_hander();
        self.signal_enable = Self::defualt_mask();
        self.pending_signal.clear();
        
        let processor_guard = get_processor();
        processor_guard.push_sum_on();
        // copy args into user stack
        let mut ptr = PROC_U_STACK_ADDR + PROC_U_STACK_SIZE;
        let mut argv = Vec::new();
        for arg in args {
            ptr = ptr - arg.len();
            unsafe{ptr.write_data(arg)};
            argv.push(ptr);
        }
        argv.push(0.into());
        let argv_ptr = ptr - argv.len() * size_of::<VirtAddr>();
        ptr = argv_ptr;
        for arg_ptr in argv.iter() {
            unsafe{ptr.write_volatile(arg_ptr)};
            ptr = ptr + size_of::<VirtAddr>();
        }
        processor_guard.pop_sum_on();

        let trap_context = TrapContext::current_ref();
        *trap_context = TrapContext::new();
        trap_context.a0 = argv.len() - 1;
        trap_context.a1 = argv_ptr.0;
        trap_context.sp = argv_ptr.0;
        trap_context.epc = entry;

        Ok(())
    }
}