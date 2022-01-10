//! Timer related sbi calls.
use crate::config::CLOCK_FREQ;
use riscv::register::time;

// trigger per 1ms
pub const TICKS_PER_SECOND  : usize = 10;
pub const MILLI_PER_SECOND  : usize = 1000;

/// Get times elaped since boot, in cycles.
pub fn get_time() -> usize {
    time::read() as usize
}

// pub fn get_time_ms() -> f64 {
//     return get_time() as f64 / (CLOCK_FREQ / MILLI_PER_SECOND) as f64;
// }

/// get milisecond since boot.
pub fn get_time_ms() -> u64 {
    return get_time() as u64 / (CLOCK_FREQ / MILLI_PER_SECOND) as u64;
}
