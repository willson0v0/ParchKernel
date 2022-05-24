//! Timer related sbi calls.
use crate::{config::{CLOCK_FREQ}, interrupt::CLINT};

// trigger per 1ms
pub const MILLI_PER_SECOND  : usize = 1000;

/// Get times elaped since boot, in cycles.
pub fn get_cycle() -> usize {
    CLINT.get_time()
}

/// get milisecond since boot.
pub fn get_time_ms() -> f64 {
    (get_time_second() as f64) * (MILLI_PER_SECOND as f64)
}

/// get second since boot.
pub fn get_time_second() -> f64 {
    (get_cycle() as f64) / (CLOCK_FREQ as f64)
}

/// TODO: check rtc stuff instead of this
pub fn get_real_time() -> f64 {
    crate::version::COMPILE_EPOCH as f64 + get_time_second()
}

pub fn get_real_time_epoch() -> usize {
    crate::version::COMPILE_EPOCH + get_time_second() as usize
}