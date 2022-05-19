use crate::{config::UUID_LENGTH, utils::Mutex};

use super::SpinMutex;
use alloc::string::String;
use lazy_static::*;
use core::hash::Hash;
use core::fmt::{Debug, Display};
use crate::alloc::string::ToString;

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

fn gen_uuid() -> u128 {
    // split 2 usize into 16 bytes;
    let mut res: u128 = rand_usize() as u128;
    res = res << 64;
    res += rand_usize() as u128;
    res
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Hash)]
pub struct UUID(pub u128);

impl UUID {
    pub fn new() -> Self {
        Self(gen_uuid())
    }

    pub fn to_bytes(&self) -> [u8; UUID_LENGTH] {
        self.0.to_be_bytes()
    }

    pub fn from_bytes(bytes: [u8; UUID_LENGTH]) -> Self {
        Self(u128::from_be_bytes(bytes))
    }
}

impl Debug for UUID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.to_string())?;
        Ok(())
    }
}

impl Display for UUID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let p0 = (self.0 >>  0) & 0xFFFFFFFFFFFF;
        let p1 = (self.0 >> 48) & 0xFFFF;
        let p2 = (self.0 >> 64) & 0xFFFF;
        let p3 = (self.0 >> 80) & 0xFFFF;
        let p4 = (self.0 >> 96) & 0xFFFFFFFF;
        write!(f, "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", p4, p3, p2, p1, p0)
    }
}