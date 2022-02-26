use crate::{process::{FileDescriptor, get_processor, push_sum_on, pop_sum_on}, mem::VirtAddr, utils::ErrorNum};

pub const SYSCALL_WRITE     : usize = 0;
pub const SYSCALL_READ      : usize = 1;
pub const SYSCALL_OPEN      : usize = 2;
pub const SYSCALL_OPENAT    : usize = 3;
pub const SYSCALL_FORK      : usize = 4;
pub const SYSCALL_EXEC      : usize = 5;
pub const SYSCALL_EXIT      : usize = 6;
pub const SYSCALL_MMAP      : usize = 7;

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
                debug!("/========== SYSCALL {} CALLED BY {:?} ==========\\", stringify!($syscall_name), process.pid);
            }
            $(
                verbose!("{:>25} = {:?}", stringify!{$y}, $y);
            )+
            let ret = $syscall_name($($y),+);
            if do_trace {
                debug!("\\= SYSCALL {} CALLED BY {:?} RESULT {:<10?} =/", stringify!($syscall_name), process.pid, ret);
            }
            ret
        }
    };
}

pub fn syscall(syscall_id: usize, args: [usize; 6]) -> Result<usize, ErrorNum> {
    match syscall_id {
        SYSCALL_WRITE => CALL_SYSCALL!(sys_write, FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2], args[3]),
        SYSCALL_READ => CALL_SYSCALL!(sys_read, FileDescriptor::from(args[0]), VirtAddr::from(args[1]), args[2], args[3]),
        _ => CALL_SYSCALL!(sys_unknown, syscall_id)
    }
}

pub fn sys_write(fd: FileDescriptor, buf: VirtAddr, length: usize, offset: usize) -> Result<usize, ErrorNum> {
    let proc = get_processor().current().unwrap();
    let pcb_inner = proc.get_inner();
    let file = pcb_inner.fd.get(&fd).ok_or(ErrorNum::EBADFD)?.clone();
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
    let file = pcb_inner.fd.get(&fd).ok_or(ErrorNum::EBADFD)?.clone();
    // TODO: register MMAP if needed
    let res = file.read(length, offset)?;
    push_sum_on();
    unsafe {buf.write_data(res)};
    pop_sum_on();
    Ok(length)
}

pub fn sys_unknown(syscall_id:usize) -> Result<usize, ErrorNum> {
    error!("Unknown syscall id {}", syscall_id);
    Err(ErrorNum::ENOSYS)
}