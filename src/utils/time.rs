//! Timer related sbi calls.
use crate::{config::{CLOCK_FREQ}, interrupt::CLINT};

// trigger per 1ms
pub const MILLI_PER_SECOND  : usize = 1000;

/// Get times elaped since boot, in cycles.
pub fn get_time() -> usize {
    CLINT.get_time()
}

/// get milisecond since boot.
pub fn get_time_ms() -> f64 {
    (get_time_second() as f64) * (MILLI_PER_SECOND as f64)
}

/// get second since boot.
pub fn get_time_second() -> f64 {
    (get_time() as f64) / (CLOCK_FREQ as f64)
}
