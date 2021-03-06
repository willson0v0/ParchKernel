#[macro_use]
mod fmt_io;

#[macro_use]
pub mod marcos;

mod panic_handler;
// mod uart;
mod lock;
pub mod time;
mod error;
pub mod riscv;
pub mod elf_rs_wrapper;
pub mod range;
mod random;
mod kprint;

pub use random::{
    rand_usize,
    UUID
};

pub use lock::{
    SpinMutex,
    MutexGuard,
    Mutex,
    SpinRWLock,
    RWLockReadGuard,
    RWLockWriteGuard,
    RWLock
};

// pub use uart::{
//     Uart,
//     UART0
// };

pub use fmt_io::{
    print,
    print_no_lock,
    log,
    LogLevel,
};

pub use error::{
    ErrorNum
};

pub use kprint::K_PRINT_HANDLER;

pub fn cast_bytes<T: Sized + Copy>(bytes: alloc::vec::Vec<u8>) -> Result<T, ErrorNum> {
    if bytes.len() != core::mem::size_of::<T>() {
        return Err(ErrorNum::ENOTALIGNED);
    }
    Ok(unsafe {core::ptr::read_unaligned(bytes.as_ptr() as *const _)})
}