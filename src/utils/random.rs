use crate::{config::UUID_LENGTH, utils::Mutex};

use super::SpinMutex;
use lazy_static::*;

lazy_static!{
    static ref RAND_STATE: XorShiftState = XorShiftState { inner: SpinMutex::new("rand state", XorShiftStateInner::new()) };
}

struct XorShiftState {
    pub inner: SpinMutex<XorShiftStateInner>
}

struct XorShiftStateInner {
    pub x: [usize; 2]
}

impl XorShiftStateInner {
    pub fn new() -> Self {
        Self {
            x: [
                super::time::get_time() % crate::config::CLOCK_FREQ, 
                super::time::get_real_time_epoch()
            ]
        }
    }
}

pub fn rand_usize() -> usize {
    let mut state = RAND_STATE.inner.acquire();
    let mut t = state.x[0];
    let s = state.x[1];
    state.x[0] = s;
    t ^= t << 23;
    t ^= t >> 18;
    t ^= s ^ (s >> 5);
    state.x[1] = t;
    t.wrapping_add(s)
}

fn gen_uuid() -> [u8; UUID_LENGTH] {
    // split 2 usize into 16 bytes;
    let b: [[u8; 8]; 2] = [rand_usize().to_be_bytes(), rand_usize().to_be_bytes()];
    b.concat().try_into().unwrap()
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
pub struct UUID {
    bytes: [u8; UUID_LENGTH]
}

impl UUID {
    pub fn new() -> Self {
        Self {
            bytes: gen_uuid()
        }
    }

    pub fn to_bytes(&self) -> [u8; UUID_LENGTH] {
        self.bytes
    }

    pub fn from_bytes(bytes: [u8; UUID_LENGTH]) -> Self {
        Self {
            bytes
        }
    }
}