use core::mem::size_of;

use alloc::{vec::Vec, sync::Arc, string::ToString};

use crate::{process::{FileDescriptor, get_processor, push_sum_on, pop_sum_on, enqueue, ProcessControlBlock, ProcessStatus, ProcessID, get_process, SignalNum, free_current}, mem::{VirtAddr, VMASegment, SegmentFlags, ManagedSegment, VPNRange}, utils::{ErrorNum, Mutex}, fs::{Path, open, OpenMode}, interrupt::trap_context::TrapContext};

use super::{syscall_num::*, types::{self, MMAPProt, MMAPFlag}};

pub fn syscall(syscall_id: usize, args: [usize; 6]) -> Result<usize, ErrorNum> {
    let do_trace = get_processor().current().unwrap().get_inner().trace_enabled[syscall_id];
    match syscall_id {
        SYSCALL_WRITE       => CALL_SYSCALL!(do_trace, sys_write      , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_READ        => CALL_SYSCALL!(do_trace, sys_read       , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_OPEN        => CALL_SYSCALL!(do_trace, sys_open       , VirtAddr::from(args[0]), args[1]),
        SYSCALL_OPENAT      => CALL_SYSCALL!(do_trace, sys_openat     , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_CLOSE       => CALL_SYSCALL!(do_trace, sys_close      , FileDescriptor::from(args[0])),
        SYSCALL_DUP         => CALL_SYSCALL!(do_trace, sys_dup        , FileDescriptor::from(args[0])),
        SYSCALL_FORK        => CALL_SYSCALL!(do_trace, sys_fork       ),
        SYSCALL_EXEC        => CALL_SYSCALL!(do_trace, sys_exec       , VirtAddr::from(args[0]), VirtAddr::from(args[1])),
        SYSCALL_EXIT        => CALL_SYSCALL!(do_trace, sys_exit       , args[0] as isize),
        SYSCALL_MMAP        => CALL_SYSCALL!(do_trace, sys_mmap       , VirtAddr::from(args[0]), args[1], MMAPProt::from_bits(args[2]).ok_or(ErrorNum::EINVAL)?, MMAPFlag::from_bits(args[3]).ok_or(ErrorNum::EINVAL)?, FileDescriptor::from(args[4]), args[5]),
        SYSCALL_WAITPID     => CALL_SYSCALL!(do_trace, sys_waitpid    , args[0] as isize, VirtAddr::from(args[1])),
        SYSCALL_SIGNAL      => CALL_SYSCALL!(do_trace, sys_signal     , ProcessID(args[0]), args[1]),
        SYSCALL_SIGACTION   => CALL_SYSCALL!(do_trace, sys_sigaction  , args[0], VirtAddr::from(args[1])),
        SYSCALL_SIGRETURN   => CALL_SYSCALL!(do_trace, sys_sigreturn  ),
        SYSCALL_GETCWD      => CALL_SYSCALL!(do_trace, sys_getcwd     , VirtAddr::from(args[0]), args[1]),
        SYSCALL_CHDIR       => CALL_SYSCALL!(do_trace, sys_chdir      , VirtAddr::from(args[0])),
        SYSCALL_SBRK        => CALL_SYSCALL!(do_trace, sys_sbrk       , args[0] as isize),
        _ => CALL_SYSCALL!(true, sys_unknown, syscall_id)
    }
}

pub fn sys_write(fd: FileDescriptor, buf: VirtAddr, length: usize) -> Result<usize, ErrorNum> {
    let file = get_processor().current().unwrap().get_inner().get_file(fd)?.clone();
    // TODO: register MMAP if needed
    push_sum_on();
    let data = unsafe{buf.read_data(length)};
    pop_sum_on();
    file.write(data)?;
    Ok(length)
}

pub fn sys_read(fd: FileDescriptor, buf: VirtAddr, length: usize) -> Result<usize, ErrorNum> {
    let file = get_processor().current().unwrap().get_inner().get_file(fd)?.clone();
    // TODO: register MMAP if needed
    let res = file.read(length)?;
    push_sum_on();
    unsafe {buf.write_data(res)};
    pop_sum_on();
    Ok(length)
}

pub fn sys_open(path: VirtAddr, open_mode: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let open_mode = OpenMode::from_bits_truncate(open_mode);
    let path = path.read_cstr()?.0;
    let path: Path = if path.starts_with('/') {
        path.into()
    } else {
        proc_inner.cwd.concat(&path.into())
    };
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
    let child_trap_ctx: &mut TrapContext = child_inner.trap_context();
    *child_trap_ctx = TrapContext::current_ref().clone();
    child_trap_ctx.a0 = 0;
    enqueue(child.clone());
    Ok(pid)
}

pub fn sys_exec(elf_path: VirtAddr, argv: VirtAddr) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let path = elf_path.read_cstr()?.0;
    let path: Path = if path.starts_with('/') {
        path.into()
    } else {
        proc_inner.cwd.concat(&path.into())
    };
    let mut args: Vec<Vec<u8>> = Vec::new();
    let mut p = argv;
    if p.0 != 0 {
        let intr_guard = get_processor();
        push_sum_on();
        loop {
            let argv_str: VirtAddr = unsafe{ p.read_volatile() };
            if argv_str.0 == 0 {
                break;
            }
            let mut bytes = argv_str.read_cstr_raw(1023);
            bytes.push(0);
            args.push(bytes);
            p += size_of::<VirtAddr>();
        }
        pop_sum_on();
    }
    debug!("proc {} exec {:?}", proc.pid, path);
    let elf_file = open(&path, OpenMode::SYS)?.as_regular()?;
    proc_inner.exec(elf_file, args)?;
    Ok(0)
}

pub fn sys_exit(exit_code: isize) -> Result<usize, ErrorNum> {
    info!("Application {} exited with code {:}", get_processor().current().unwrap().pid, exit_code);
    free_current();
    get_processor().exit_switch(exit_code);
    unreachable!("This part should be unreachable. Go check __switch.")
}

pub fn sys_mmap(tgt_addr: VirtAddr, length: usize, prot: MMAPProt, flag: MMAPFlag, fd: FileDescriptor, offset: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    
    let tgt_pos: VirtAddr = if flag.contains(MMAPFlag::FIXED) {
        for i in VPNRange::new(tgt_addr.into(), (tgt_addr+length).to_vpn_ceil()) {
            if proc_inner.mem_layout.occupied(i) {
                return Err(ErrorNum::EADDRINUSE);
            }
        }
        tgt_addr
    } else {
        proc_inner.mem_layout.get_space(length)?.into()
    };

    if flag.contains(MMAPFlag::ANONYMOUS) {
        if fd != FileDescriptor::from(usize::MAX) {
            return Err(ErrorNum::EINVAL);
        }
        
        proc_inner.mem_layout.register_segment(ManagedSegment::new(VPNRange::new(
            tgt_pos.into(), (tgt_pos+length).to_vpn_ceil().into()), 
            prot.into(), 
            None,
            length
        ));
        proc_inner.mem_layout.do_map();
        Ok(VirtAddr::from(tgt_pos).0)

    } else {
        let mmap_file = proc_inner.get_file(fd)?.as_regular()?;
        let stat = mmap_file.stat()?;
        if length > stat.file_size {
            return Err(ErrorNum::EOOR)
        }
        let seg_flag: SegmentFlags = prot.into();
        if seg_flag.contains(SegmentFlags::W) && !stat.open_mode.contains(OpenMode::WRITE) {
            return Err(ErrorNum::EPERM);
        }
        if seg_flag.contains(SegmentFlags::X) && !stat.open_mode.contains(OpenMode::EXEC) {
            return Err(ErrorNum::EPERM);
        }
        proc_inner.mem_layout.register_segment(VMASegment::new_at(
            tgt_pos.into(),
            mmap_file,
            seg_flag,
            offset,
            length
        )?);
        proc_inner.mem_layout.do_map();
        Ok(VirtAddr::from(tgt_pos).0)
    }
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
            // verbose!("Waitpid not found");
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

pub fn sys_getcwd(buf: VirtAddr, length: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let proc_inner = proc.get_inner();
    let path = format!("{:?}", proc_inner.cwd);
    let mut path = path.into_bytes();
    // additional 1 byte for \0
    if path.len() >= length-1 {
        path = path[..length-1].to_vec();
    }
    path.push(0);
    let int_guard = get_processor();
    push_sum_on();
    unsafe{buf.write_data(path);}
    pop_sum_on();
    Ok(buf.0)
}

pub fn sys_chdir(buf: VirtAddr) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let path = buf.read_cstr()?.0;
    let path: Path = if path.starts_with('/') {
        path.into()
    } else {
        proc_inner.cwd.concat(&path.into())
    };
    open(&path, OpenMode::SYS)?.as_dir()?; // check if it's actually a dir
    proc_inner.cwd = path;
    Ok(0)
}

pub fn sys_sbrk(increment: isize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let data_segment = proc_inner.mem_layout.get_segment((proc_inner.data_end - 1).into())?.as_managed()?;
    let res = if increment > 0 {
        data_segment.grow(increment as usize, &mut proc_inner.mem_layout.pagetable)?
    } else if increment < 0 {
        data_segment.shrink(-increment as usize, &mut proc_inner.mem_layout.pagetable)?
    } else {
        data_segment.get_end_va()
    };
    Ok(res.0)
}

pub fn sys_unknown(syscall_id:usize) -> Result<usize, ErrorNum> {
    error!("Unknown syscall id {}", syscall_id);
    Err(ErrorNum::ENOSYS)
}