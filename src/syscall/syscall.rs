use alloc::{vec::Vec, sync::Arc};

use crate::{process::{FileDescriptor, get_processor, push_sum_on, pop_sum_on, enqueue, ProcessControlBlock, ProcessStatus, ProcessID, get_process, SignalNum, free_current}, mem::{VirtAddr, VMASegment, SegmentFlags}, utils::ErrorNum, fs::{Path, open, OpenMode}, interrupt::trap_context::TrapContext};

pub const SYSCALL_WRITE     : usize =  0;
pub const SYSCALL_READ      : usize =  1;
pub const SYSCALL_OPEN      : usize =  2;
pub const SYSCALL_OPENAT    : usize =  3;
pub const SYSCALL_CLOSE     : usize =  4;
pub const SYSCALL_DUP       : usize =  5;
pub const SYSCALL_FORK      : usize =  6;
pub const SYSCALL_EXEC      : usize =  7;
pub const SYSCALL_EXIT      : usize =  8;
pub const SYSCALL_MMAP      : usize =  9;
pub const SYSCALL_SIGNAL    : usize = 10;
pub const SYSCALL_WAITPID   : usize = 11;
pub const SYSCALL_SIGACTION : usize = 12;
pub const SYSCALL_SIGRETURN : usize = 13;

macro_rules! CALL_SYSCALL {
    ( $syscall_name: expr ) => {
        {
            let process = crate::process::get_processor().current().unwrap();
            let do_trace = process.get_inner().trace;
            if do_trace {
                debug!("/========== SYSCALL {} CALLED BY {:?} ==========\\", stringify!($syscall_name), process.pid);
            }
            let ret = $syscall_name();
            if do_trace {
                debug!("\\= SYSCALL {} CALLED BY {} RESULT {:<10?} =/", stringify!($syscall_name), process.pid, ret);
            }
            ret
        }
    };
    ( $syscall_name: expr, $($y:expr),+ ) => {
        {
            let process = crate::process::get_processor().current().unwrap();
            let do_trace = process.get_inner().trace;
            if do_trace {
                debug!("SYSCALL {} CALLED BY {:?}", stringify!($syscall_name), process.pid);
                $(
                    verbose!("{:>25} = {:?}", stringify!{$y}, $y);
                )+
            }
            let ret = $syscall_name($($y),+);
            if do_trace {
                debug!("SYSCALL {} CALLED BY {:?} RESULT {:?}", stringify!($syscall_name), process.pid, ret);
            }
            ret
        }
    };
}

pub fn syscall(syscall_id: usize, args: [usize; 6]) -> Result<usize, ErrorNum> {
    match syscall_id {
        SYSCALL_WRITE       => CALL_SYSCALL!(sys_write      , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2], args[3]),
        SYSCALL_READ        => CALL_SYSCALL!(sys_read       , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2], args[3]),
        SYSCALL_OPEN        => CALL_SYSCALL!(sys_open       , VirtAddr::from(args[0]), args[1]),
        SYSCALL_OPENAT      => CALL_SYSCALL!(sys_openat     , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_CLOSE       => CALL_SYSCALL!(sys_close      , FileDescriptor::from(args[0])),
        SYSCALL_DUP         => CALL_SYSCALL!(sys_dup        , FileDescriptor::from(args[0])),
        SYSCALL_FORK        => CALL_SYSCALL!(sys_fork       ),
        SYSCALL_EXEC        => CALL_SYSCALL!(sys_exec       , VirtAddr::from(args[0]), VirtAddr::from(args[1])),
        SYSCALL_EXIT        => CALL_SYSCALL!(sys_exit       , args[0] as isize),
        SYSCALL_MMAP        => CALL_SYSCALL!(sys_mmap       , FileDescriptor::from(args[0]), SegmentFlags::from_bits_truncate(args[1])),
        SYSCALL_WAITPID     => CALL_SYSCALL!(sys_waitpid    , args[0] as isize, VirtAddr::from(args[1])),
        SYSCALL_SIGNAL      => CALL_SYSCALL!(sys_signal     , ProcessID(args[0]), args[1]),
        SYSCALL_SIGACTION   => CALL_SYSCALL!(sys_sigaction  , args[0], VirtAddr::from(args[1])),
        SYSCALL_SIGRETURN   => CALL_SYSCALL!(sys_sigreturn  ),
        _ => CALL_SYSCALL!(sys_unknown, syscall_id)
    }
}

pub fn sys_write(fd: FileDescriptor, buf: VirtAddr, length: usize, offset: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let pcb_inner = proc.get_inner();
    let file = pcb_inner.get_file(fd)?;
    // TODO: register MMAP if needed
    push_sum_on();
    let data = unsafe{buf.read_data(length)};
    pop_sum_on();
    file.write(data, offset)?;
    Ok(length)
}

pub fn sys_read(fd: FileDescriptor, buf: VirtAddr, length: usize, offset: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let pcb_inner = proc.get_inner();
    let file = pcb_inner.get_file(fd)?;
    // TODO: register MMAP if needed
    let res = file.read(length, offset)?;
    push_sum_on();
    unsafe {buf.write_data(res)};
    pop_sum_on();
    Ok(length)
}

pub fn sys_open(path: VirtAddr, open_mode: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let open_mode = OpenMode::from_bits_truncate(open_mode);
    let (path, _) = path.read_cstr()?;
    let path: Path = path.into();
    let file = open(&path, open_mode)?;
    Ok(proc_inner.register_file(file)?.0)
}

pub fn sys_openat(dirfd: FileDescriptor, path: VirtAddr, open_mode: usize) -> Result<usize, ErrorNum>  {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let open_mode = OpenMode::from_bits_truncate(open_mode);
    let (path, _) = path.read_cstr()?;
    let path: Path = path.into();
    let dir_file = proc_inner.get_file(dirfd)?.as_dir()?;
    let file = dir_file.open_dir(&path, open_mode)?;
    proc_inner.register_file(file).map(|fd| fd.0)
}

pub fn sys_close(fd: FileDescriptor) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    proc_inner.close_file(fd)?;
    Ok(0)
}

pub fn sys_dup(fd: FileDescriptor) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    proc_inner.dup_file(fd).map(|fd| fd.0)
}

pub fn sys_fork() -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let child = proc.fork()?;
    let mut pcb_inner = proc.get_inner();
    let mut child_inner = child.get_inner();
    child_inner.parent = Some(Arc::downgrade(&proc));
    pcb_inner.children.insert(child.clone());
    let pid = child.pid.0;
    child_inner.trap_context().a0 = 0;
    enqueue(child.clone());
    Ok(pid)
}

pub fn sys_exec(elf_path: VirtAddr, argv: VirtAddr) -> Result<usize, ErrorNum> {
    let path: Path = elf_path.read_cstr()?.0.into();
    let mut args: Vec<Vec<u8>> = Vec::new();
    let mut p = argv;
    if p.0 != 0 {
        loop {
            let mut bytes = p.read_cstr_raw(1023);
            if bytes.len() == 0 {
                break;
            }
            bytes.push(0);
            p += bytes.len();
            args.push(bytes);
        }
    }
    let proc = get_processor().current().unwrap();
    debug!("proc {} exec {:?}", proc.pid, path);
    let elf_file = open(&path, OpenMode::SYS)?.as_regular()?;
    proc.exec(elf_file, args)?;
    Ok(0)
}

pub fn sys_exit(exit_code: isize) -> Result<usize, ErrorNum> {
    info!("Application {} exited with code {:}", get_processor().current().unwrap().pid, exit_code);
    free_current();
    get_processor().exit_switch(exit_code);
    unreachable!("This part should be unreachable. Go check __switch.")
}

pub fn sys_mmap(fd: FileDescriptor, flag: SegmentFlags) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let file = proc_inner.get_file(fd)?.as_regular()?;
    let stat = file.stat()?;
    if flag.contains(SegmentFlags::W) && !stat.open_mode.contains(OpenMode::WRITE) {
        return Err(ErrorNum::EPERM);
    }
    if flag.contains(SegmentFlags::X) && !stat.open_mode.contains(OpenMode::EXEC) {
        return Err(ErrorNum::EPERM);
    }
    let tgt_pos = proc_inner.mem_layout.get_space(stat.file_size)?;
    proc_inner.mem_layout.register_segment(VMASegment::new_at(
        tgt_pos,
        file,
        flag
    ));
    proc_inner.mem_layout.do_map();
    Ok(VirtAddr::from(tgt_pos).0)
}

pub fn sys_waitpid(pid: isize, exit_code: VirtAddr) -> Result<usize, ErrorNum> {
    info!("Waitpid called for {} from {}", pid, get_processor().current().unwrap().pid);
    loop {
        let proc = get_processor().current().unwrap();
        let mut pcb_inner = proc.get_inner();

        if !pcb_inner.pending_signal.is_empty() {
            warning!("Recv Signal, Waitpid failed.");
            return Err(ErrorNum::EINTR);
        }

        let mut body_bag: Option<Arc<ProcessControlBlock>> = None;
        for child in pcb_inner.children.clone().into_iter() {
            if ((pid == -1) || (pid as usize == child.pid.0)) && child.get_inner().status == ProcessStatus::Zombie {
                body_bag = Some(child);
                break;
            }
        }

        if let Some(corpse) = body_bag {
            let corpse = pcb_inner.children.take(&corpse).unwrap();
            let corpse_inner = corpse.get_inner();
            assert!(Arc::strong_count(&corpse) == 1, "Zombie {:?} was referenced by something else.", corpse.pid);
            info!("Zombie {:?} was killed.", corpse.pid);
            unsafe{exit_code.write_volatile(&corpse_inner.exit_code.unwrap());}
            return Ok(corpse.pid.0);
        } else {
            drop(pcb_inner);
            verbose!("Waitpid not found");
            get_processor().suspend_switch();
        }
    }
}

pub fn sys_signal(target_pid: ProcessID, signum: usize) -> Result<usize, ErrorNum> {
    let to_recv = get_process(target_pid)?;
    let mut to_recv_inner = to_recv.get_inner();
    // TODO: check permission
    let signal = SignalNum::try_from(signum)?;
    to_recv_inner.recv_signal(signal)?;
    Ok(0)
}

pub fn sys_sigaction(signum: usize, handler: VirtAddr) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let signal = SignalNum::try_from(signum)?;
    proc_inner.signal_handler.insert(signal, handler);
    Ok(0)
}

pub fn sys_sigreturn() -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    if let Some(old_ctx) = proc_inner.signal_contexts.pop() {
        debug!("Overwriting TrapContext from sigreturn...");
        let trap_ctx = TrapContext::current_ref();
        *trap_ctx = old_ctx;
        Ok(0)
    } else {
        error!("sys_sigreturn called when no signal context was saved");
        Err(ErrorNum::ENOSIG)
    }
}

pub fn sys_unknown(syscall_id:usize) -> Result<usize, ErrorNum> {
    error!("Unknown syscall id {}", syscall_id);
    Err(ErrorNum::ENOSYS)
}