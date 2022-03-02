/// Core Local Interrupt
/// 
use crate::mem::PhysAddr;
use crate::config::CLINT_ADDR;

pub static CLINT: Clint = Clint::new(CLINT_ADDR);

pub struct Clint {
    address: PhysAddr,
}

impl Clint {
    pub const fn new(address: PhysAddr) -> Self {
        Clint {
            address
        }
    }

    pub fn get_time(&self) -> usize {
        unsafe {
            (self.address + 0xBFF8).read_volatile()
        } 
    }

    pub fn set_mtimecmp(&self, hart: usize, nxt_int: usize) {
        unsafe {
            (self.address + 0x4000 + 8 * hart).write_volatile(&nxt_int);
        }
    }
}