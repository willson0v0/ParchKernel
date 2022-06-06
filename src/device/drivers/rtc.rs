

use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::device::DeviceTree;

use crate::{device::device_manager::Driver, mem::PhysAddr};
use crate::utils::{ErrorNum, RWLock, UUID};
use core::fmt::Debug;
use core::mem::size_of;

/// Driver for google goldfish rtc device. Typically mapped at 0x101000
/// 0x00 TIME_LOW
/// 0x04 TIME_HI
/// 0x08 ALARM_LO   // The device will not raise IRQ, these are for compatibility
/// 0x0C ALARM_HI   // The device will not raise IRQ, these are for compatibility
/// 0x10 CLEAR_INT
pub struct RTC {
    addr: PhysAddr, 
}

enum_with_tryfrom_usize!{
    #[repr(usize)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum IOCtlOp {
        ReadTime = 1,
    }
}

impl Debug for RTC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "RTC @ {:?}", self.addr)
    }
}

impl RTC {
    fn read_time(&self) -> u64 {
        let time_low: u32 = unsafe{(self.addr + 0x00).read_volatile()};
        let time_hi: u32 = unsafe{(self.addr + 0x04).read_volatile()};
        time_low as u64 + ((time_hi as u64) << 32)
    }
}

impl Driver for RTC {
    fn new(dev_tree: DeviceTree) -> Result<Vec<(UUID, Arc<(dyn Driver + 'static)>)>, ErrorNum> where Self: Sized {
        let mut res = Vec::new();
        let nodes = dev_tree.serach_compatible("google,goldfish-rtc")?;
        for node in nodes {
            let node_r = node.acquire_r();
            let uuid = node_r.driver;
            verbose!("RTC Driver found device: {}, uuid {}.", node_r.unit_name, uuid);
            let reg = node_r.reg_value()?;
            verbose!("MMIO Range: start 0x{:x}, length: 0x{:x}", reg[0].address, reg[0].size);
            // assert size?
            res.push((uuid, Arc::new(Self{
                addr: reg[0].address.into(),
                // int_parent: node_r.get_value("interrupt-parent")?.get_u32()?,
                // int_id: node_r.get_value("interrupts")?.get_u32()?,
            }).as_driver()))
        }
        Ok(res)
    }

    fn initialize(&self) -> Result<(), ErrorNum> {
        Ok(())
    }

    fn terminate(&self) {
        ()
    }

    fn ioctl(&self, op: usize, data: Vec<u8>) -> Result<Vec<u8>, ErrorNum> {
        let op: IOCtlOp = op.try_into()?;
        // sanity check
        if size_of::<()>() != data.len() {
            return Err(ErrorNum::EINVAL);
        }
        if op == IOCtlOp::ReadTime {
            Ok(self.read_time().to_le_bytes().to_vec())
        } else {
            Err(ErrorNum::ENOSYS)
        }
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn as_driver<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn Driver> {
        self
    }

    fn handle_int(&self) -> Result<(), ErrorNum> {
        panic!("No Int for RTC!")
    }

    fn as_int_controller<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::device::device_manager::IntController>, ErrorNum> {
        Err(ErrorNum::ENOTINTC)
    }

    fn write(&self, _data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
        if length == size_of::<u64>() {
            Ok(self.read_time().to_le_bytes().to_vec())
        } else {
            Err(ErrorNum::EPERM)
        }
    }
}