use core::{mem::size_of};

use alloc::{vec::Vec, sync::Arc, collections::LinkedList, borrow::ToOwned, string::String};

use crate::{config::PHYS_END_ADDR, fs::{FileType, OpenMode, Path, Permission, delete, make_file, new_pipe, open, open_at}, interrupt::trap_context::TrapContext, mem::{VirtAddr, VMASegment, SegmentFlags, ManagedSegment, VPNRange, stat_mem, MMAPType}, process::{FileDescriptor, get_processor, push_sum_on, pop_sum_on, enqueue, ProcessStatus, ProcessID, get_process, SignalNum, free_current}, utils::{ErrorNum}};

use super::{syscall_num::*, types::{MMAPProt, MMAPFlag, SyscallDirent, SyscallStat}};

pub fn syscall(syscall_id: usize, args: [usize; 6]) -> Result<usize, ErrorNum> {
    let do_trace = get_processor().current().unwrap().get_inner().trace_enabled[syscall_id];
    match syscall_id {
        SYSCALL_WRITE       => CALL_SYSCALL!(do_trace, sys_write        , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_READ        => CALL_SYSCALL!(do_trace, sys_read         , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_OPEN        => CALL_SYSCALL!(do_trace, sys_open         , VirtAddr::from(args[0]), args[1]),
        SYSCALL_OPENAT      => CALL_SYSCALL!(do_trace, sys_openat       , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_CLOSE       => CALL_SYSCALL!(do_trace, sys_close        , FileDescriptor::from(args[0])),
        SYSCALL_DUP         => CALL_SYSCALL!(do_trace, sys_dup          , FileDescriptor::from(args[0])),
        SYSCALL_FORK        => CALL_SYSCALL!(do_trace, sys_fork         ),
        SYSCALL_EXEC        => CALL_SYSCALL!(do_trace, sys_exec         , VirtAddr::from(args[0]), VirtAddr::from(args[1])),
        SYSCALL_EXIT        => CALL_SYSCALL!(do_trace, sys_exit         , args[0] as isize),
        SYSCALL_MMAP        => CALL_SYSCALL!(do_trace, sys_mmap         , VirtAddr::from(args[0]), args[1], MMAPProt::from_bits(args[2]).ok_or(ErrorNum::EINVAL)?, MMAPFlag::from_bits(args[3]).ok_or(ErrorNum::EINVAL)?, FileDescriptor::from(args[4]), args[5]),
        SYSCALL_WAITPID     => CALL_SYSCALL!(do_trace, sys_waitpid      , args[0] as isize, VirtAddr::from(args[1])),
        SYSCALL_SIGNAL      => CALL_SYSCALL!(do_trace, sys_signal       , ProcessID(args[0]), args[1]),
        SYSCALL_SIGACTION   => CALL_SYSCALL!(do_trace, sys_sigaction    , args[0], VirtAddr::from(args[1])),
        SYSCALL_SIGRETURN   => CALL_SYSCALL!(do_trace, sys_sigreturn    ),
        SYSCALL_GETCWD      => CALL_SYSCALL!(do_trace, sys_getcwd       , VirtAddr::from(args[0]), args[1]),
        SYSCALL_CHDIR       => CALL_SYSCALL!(do_trace, sys_chdir        , VirtAddr::from(args[0])),
        SYSCALL_SBRK        => CALL_SYSCALL!(do_trace, sys_sbrk         , args[0] as isize),
        SYSCALL_GETDENTS    => CALL_SYSCALL!(do_trace, sys_getdents     , FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2]),
        SYSCALL_PIPE        => CALL_SYSCALL!(do_trace, sys_pipe         , VirtAddr::from(args[0])),
        SYSCALL_SYSSTAT     => CALL_SYSCALL!(do_trace, sys_sysstat      , VirtAddr::from(args[0])),
        SYSCALL_IOCTL       => CALL_SYSCALL!(do_trace, sys_ioctl        , FileDescriptor::from(args[0]), args[1], VirtAddr::from(args[2]), args[3], VirtAddr::from(args[4]), args[5]),
        SYSCALL_DELETE      => CALL_SYSCALL!(do_trace, sys_delete       , VirtAddr::from(args[0])),
        SYSCALL_MKDIR       => CALL_SYSCALL!(do_trace, sys_mkdir        , VirtAddr::from(args[0]), Permission::from_bits_truncate(args[1] as u16)),
        SYSCALL_SEEK        => CALL_SYSCALL!(do_trace, sys_seek         , FileDescriptor::from(args[0]), args[1]),
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
    let length = res.len();
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    if buf.write_user_data(&proc_inner.mem_layout.pagetable, res).is_err() {
        proc_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
    }
    Ok(length)
}

pub fn sys_open(path: VirtAddr, open_mode: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let proc_inner = proc.get_inner();
    let open_mode = OpenMode::from_bits_truncate(open_mode);
    let path = path.read_cstr()?.0;
    let path: Path = if path.starts_with('/') {
        path.into()
    } else {
        proc_inner.cwd.concat(&path.into())
    };
    // path.reduce();
    // open procfs need self inner, so unlock first
    drop(proc_inner);
    let file = open(&path, open_mode)?;
    Ok(get_processor().current().unwrap().get_inner().register_file(file)?.0)
}

pub fn sys_openat(dirfd: FileDescriptor, path: VirtAddr, open_mode: usize) -> Result<usize, ErrorNum>  {
    let proc = get_processor().current().unwrap();
    let proc_inner = proc.get_inner();
    let open_mode = OpenMode::from_bits_truncate(open_mode);
    let (path, _) = path.read_cstr()?;
    let path: Path = path.into();
    let dir_file = proc_inner.get_file(dirfd)?.as_dir()?;
    // open procfs need self inner, so unlock first
    drop(proc_inner);
    let file = open_at(dir_file.as_file(), &path, open_mode)?;
    get_processor().current().unwrap().get_inner().register_file(file).map(|fd| fd.0)
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
    let mut pcb_inner = proc.get_inner();   // always lock parent first, then child
    let mut child_inner = child.get_inner();
    child_inner.parent = Some(Arc::downgrade(&proc));
    pcb_inner.children.push_back(child.clone());
    let pid = child.pid.0;
    child_inner.trap_context().a0 = 0;
    child_inner.trap_context().a1 = 0;
    enqueue(child.clone());
    Ok(pid)
}

pub fn sys_exec(elf_path: VirtAddr, argv: VirtAddr) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let path = elf_path.read_cstr()?.0;
    debug!("proc {} exec {:?}", proc.pid, path);
    let path: Path = if path.starts_with('/') {
        path.into()
    } else {
        proc_inner.cwd.concat(&path.into())
    };
    verbose!("Init exec path: {:?}", path);
    let mut args: Vec<Vec<u8>> = Vec::new();

    let mut exec_path = path.clone();
    // check if it's shabang
    let file = open(&path, OpenMode::READ | OpenMode::EXEC)?;
    let shebang = file.read(2)?;
    if shebang[0] == b'#' && shebang[1] == b'!' {
        info!("shabang discoverd.");
        
        let mut shebang_exec: Vec<u8> = Vec::new();
        loop {
            let byte = file.read(1)?[0];
            if byte == b' ' {
                continue;
            } else if byte != b'\r' && byte != b'\n' {
                shebang_exec.push(byte);
            } else {
                break;
            }
        }
        let shebang_exec_str = String::from_utf8(shebang_exec.clone()).map_err(|_| ErrorNum::ENOENT)?;
        exec_path = shebang_exec_str.into();
        
        shebang_exec.push(0);
        args.push(shebang_exec);
    }

    let mut name_bytes = format!("{:?}", path).into_bytes();
    name_bytes.push(b'\0');
    args.push(name_bytes);
    let mut p = argv;
    if p.0 != 0 {
        let _intr_guard = get_processor();
        push_sum_on();
        loop {
            let argv_str: VirtAddr = unsafe{ p.read_volatile() };
            if argv_str.0 == 0 {break;}
            let mut bytes = argv_str.read_cstr_raw(1023);
            bytes.push(0);
            args.push(bytes);
            p += size_of::<VirtAddr>();
        }
        pop_sum_on();
    }

    for (idx, s) in args.iter().enumerate() {
        debug!("argv {} : {:?}", idx, String::from_utf8(s.clone()));
    }

    let elf_file = open(&exec_path, OpenMode::SYS)?.as_regular()?;
    let arg_count = args.len();
    proc_inner.exec(elf_file, args)?;
    Ok(arg_count)
}

pub fn sys_exit(exit_code: isize) -> Result<usize, ErrorNum> {
    let processor = get_processor();
    info!("Application {} exited with code {:}", processor.current().unwrap().pid, exit_code);
    // un-register it from process manager
    free_current();
    processor.exit_switch(exit_code);
    // unreachable!("This part should be unreachable. Go check __switch.")
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
            length,
            if flag.contains(MMAPFlag::SHARED) {
                MMAPType::Shared
            } else {
                MMAPType::Private
            }
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

        let mut zombies = pcb_inner.children.drain_filter(
            |child| -> bool {
                child.get_inner().status == ProcessStatus::Zombie
            }
        ).collect::<LinkedList<_>>();

        if let Some(corpse) = zombies.pop_front() {
            pcb_inner.children.append(&mut zombies);
            let corpse_inner = corpse.get_inner();
            // make sure its data got released
            // 1 here, 2 in scheduler context (maybe)
            // NOTE: in multicore, it can be referenced by other cores.
            // assert!(Arc::strong_count(&corpse) <= 2, "Zombie {:?} was referenced by something else, strong_count = {}", corpse.pid, Arc::strong_count(&corpse));
            info!("Zombie {:?} was killed.", corpse.pid);
            if exit_code.0 != 0 {
                if exit_code.write_user(&pcb_inner.mem_layout.pagetable, &corpse_inner.exit_code.unwrap()).is_err() {
                    pcb_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
                    return Err(ErrorNum::EPERM);
                }
            }
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
    let mut proc_inner = proc.get_inner();
    let path = format!("{:?}", proc_inner.cwd);
    let mut path = path.into_bytes();
    // additional 1 byte for \0
    if path.len() >= length-1 {
        path = path[..length-1].to_vec();
    }
    path.push(0);
    let _int_guard = get_processor();
    if buf.write_user_data(&proc_inner.mem_layout.pagetable, path).is_err() {
        proc_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
    }
    Ok(buf.0)
}

pub fn sys_chdir(buf: VirtAddr) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let path = buf.read_cstr()?.0;
    let mut path: Path = if path.starts_with('/') {
        path.into()
    } else {
        proc_inner.cwd.concat(&path.into())
    };
    open(&path, OpenMode::SYS)?.as_dir()?; // check if it's actually a dir
    path.reduce();
    proc_inner.cwd = path;
    Ok(0)
}

pub fn sys_sbrk(increment: isize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    let data_segment = proc_inner.mem_layout.get_segment((proc_inner.data_end - 1).into())?.as_program()?;
    data_segment.alter_size(increment, &mut proc_inner.mem_layout.pagetable)
}

pub fn sys_getdents(fd: FileDescriptor, buf: VirtAddr, count: usize) -> Result<usize, ErrorNum>{
    let proc = get_processor().current().unwrap();
    let proc_inner = proc.get_inner();
    let dir_file = proc_inner.get_file(fd)?.as_dir()?;

    // avoid procfs deadlock
    drop(proc_inner);
    let dirents = dir_file.read_dirent()?;
    let mut proc_inner = proc.get_inner();
    
    let mut written = 0;
    for (idx, dirent)in dirents.iter().enumerate() {
        if idx >= count {
            break;
        }
        let syscall_dirent = SyscallDirent::from(dirent.to_owned());
        if (buf + idx * size_of::<SyscallDirent>()).write_user(&(proc_inner.mem_layout.pagetable), &syscall_dirent).is_err() {
            proc_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
            return Err(ErrorNum::EPERM);
        }
        written += 1;
    }
    Ok(written)
}

pub fn sys_pipe(ret: VirtAddr) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    
    let (r, w) = new_pipe();
    let r_fd = proc_inner.register_file(r)?;
    let w_fd = proc_inner.register_file(w)?;

    let result = [r_fd, w_fd];
    if ret.write_user(&proc_inner.mem_layout.pagetable, &result).is_err() {
        proc_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
        Err(ErrorNum::EPERM)
    } else {
        Ok(0)
    }
}

pub fn sys_sysstat(stat_ptr: VirtAddr) -> Result<usize, ErrorNum> {
    let (fs_usage, mm_usage) = stat_mem();
    extern "C" {
        fn ekernel();
        fn skernel();
    }
    let stat = SyscallStat {
        persistant_usage: fs_usage,
        runtime_usage: mm_usage,
        kernel_usage: ekernel as usize - skernel as usize,
        total_available: PHYS_END_ADDR.0 - skernel as usize,
    };
    let proc = get_processor().current().unwrap();
    let mut proc_inner = proc.get_inner();
    if stat_ptr.write_user(&proc_inner.mem_layout.pagetable, &stat).is_err() {
        proc_inner.recv_signal(SignalNum::SIGSEGV).unwrap();
    }
    Ok(0)
}

pub fn sys_munmap() {

    todo!()
}

pub fn sys_ioctl(fd: FileDescriptor, op: usize, buf: VirtAddr, length: usize, target: VirtAddr, tgt_size: usize) -> Result<usize, ErrorNum> {
    let file = get_processor().current().unwrap().get_inner().get_file(fd)?.clone().as_char()?;
    let data = unsafe{ buf.read_data(length) };
    let res = file.ioctl(op, data)?;
    let res_len = res.len();
    if res_len > tgt_size {
        return Err(ErrorNum::EOVERFLOW);
    }
    unsafe{target.write_data(res)};
    Ok(res_len)
}

pub fn sys_delete(buf: VirtAddr) -> Result<usize, ErrorNum> {
    let (path, _) = buf.read_cstr()?;
    let path = Path::from(path);
    delete(&path)?;
    Ok(0)
}

pub fn sys_mkdir(buf: VirtAddr, permission: Permission) -> Result<usize, ErrorNum> {
    let (path, _) = buf.read_cstr()?;
    let prefix = if !path.starts_with('/') {
        get_processor().current().unwrap().get_inner().cwd.clone()
    } else {
        Path::root()
    };
    let path = prefix.concat(&Path::from(path));
    make_file(&path, permission, FileType::DIR)?;
    Ok(0)
}

pub fn sys_seek(fd: FileDescriptor, offset: usize) -> Result<usize, ErrorNum> {
    let file = get_processor().current().unwrap().get_inner().get_file(fd)?.clone().as_regular()?;
    file.seek(offset)
}

pub fn sys_unknown(syscall_id:usize) -> Result<usize, ErrorNum> {
    error!("Unknown syscall id {}", syscall_id);
    Err(ErrorNum::ENOSYS)
}