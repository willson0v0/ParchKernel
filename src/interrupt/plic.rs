use core::mem::size_of;

use crate::{config::{PLIC_ADDR}, mem::PhysAddr};
use alloc::vec::{Vec};
use crate::process::get_hart_id;
use lazy_static::*;

lazy_static!{
    pub static ref PLIC0: PLIC = PLIC::new(PLIC_ADDR) ;
}

pub struct PLIC {
    address: PhysAddr,
}

impl PLIC {
    pub fn new(address: PhysAddr) -> Self {
        PLIC {
            address
        }
    }

    pub fn enable_irqs_priority(&self, irqs: Vec<u32>) {
        for irq in irqs {
            // set irq priorities
            unsafe {
                (self.address + irq as usize * size_of::<u32>()).write_volatile(&1u32);
            }
        }
    }

    pub fn enable_irqs_hart(&self, irqs: Vec<u32>) {
        let hart = get_hart_id();
        let plic_senable = self.address + 0x2080usize + hart * 0x100usize;
        let plic_spriority = self.address + 0x201000usize + hart * 0x2000usize;
        verbose!("PLIC S Enable:   {:?}", plic_senable);
        verbose!("PLIC S Priority: {:?}", plic_spriority);
        let mut bits = 0u32;
        for irq in irqs {
            bits |= 1 << irq;
        }
        unsafe {
            // enable S-mode irq
            plic_senable.write_volatile(&bits);
            // set S-mode irq priority threshold to 0
            plic_spriority.write_volatile(&0u32);
        }
    }

    pub fn plic_claim(&self) -> u32 {
        let hart = get_hart_id();
        let plic_sclaim = self.address + 0x201004usize + hart * 0x2000usize;
        unsafe {
            return plic_sclaim.read_volatile();
        }
    }

    pub fn plic_complete(&self, irq: u32) {
        let hart = get_hart_id();
        let plic_sclaim = self.address + 0x201004usize + hart * 0x2000usize;
        unsafe {
            plic_sclaim.write_volatile(&irq)
        }
    }
}