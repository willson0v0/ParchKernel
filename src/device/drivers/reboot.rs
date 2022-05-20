use alloc::{boxed::Box, sync::Arc, vec::Vec};

use crate::{device::{device_manager::Driver, device_tree::DTBPropertyValue}, mem::PhysAddr, utils::{RWLock, UUID}};
use core::{fmt::Debug, mem::size_of};
use crate::utils::ErrorNum;

/// This is a generic poweroff dirver using syscon to map the poweroff register.
/// 32 bit access only
pub struct Reboot {
    syscon_reg: PhysAddr,
    reboot_magic: u32
}

impl Debug for Reboot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Reboot driver, syscon reg @ {:?}", self.syscon_reg)
    }
}

enum_with_tryfrom_usize!{
    #[repr(usize)]
    pub enum IOCtlOp {
        Shutdown = 1,
    }
}

impl Reboot {
    pub fn reboot(&self) {
        unsafe {
            self.syscon_reg.write_volatile(&self.reboot_magic)
        }
    }
}

impl Driver for Reboot {
    fn new(dev_tree: crate::device::DeviceTree) -> Result<alloc::vec::Vec<(crate::utils::UUID, alloc::sync::Arc<dyn Driver>)>, crate::utils::ErrorNum> where Self: Sized {
        match dev_tree.serach_compatible("syscon-reboot")?.as_slice() {
            [node_guard] => {
                let node = node_guard.acquire_r();
                let uuid = node.driver;
                let phandle = match node.get_value("regmap")? {
                    DTBPropertyValue::UInt32(phandle) => phandle,
                    _ => return Err(ErrorNum::EBADDTB)
                };
                let regmap_node = match dev_tree.search("phandle", DTBPropertyValue::UInt32(phandle))?.as_slice() {
                    [node] => node.clone(),
                    _ => return Err(ErrorNum::EBADDTB)
                };
                let syscon_reg: PhysAddr = regmap_node.acquire_r().reg_value()?[0].address.into();
                let shutdown_magic = node.get_value("value")?.get_u32()?;
                verbose!("Creating driver instance for reboot, syscon reg {:?}, uuid {}, shutdown magic {}", syscon_reg, uuid, shutdown_magic);
                // sanity check
                let res = Reboot {
                    syscon_reg,
                    reboot_magic: shutdown_magic,
                };
                return Ok(vec![(uuid, Arc::new(res))]);
            },
            _ => panic!("No reboot or multiple reboot in dev_tree")
        };
    }

    fn initialize(&self) -> Result<(), crate::utils::ErrorNum> {
        Ok(())
    }

    fn terminate(&self) {
        
    }

    fn ioctl(&self, op: usize, data: Vec<u8>) -> Result<Vec<u8>, ErrorNum> {
        let op: IOCtlOp = op.try_into()?;
        // sanity check
        if size_of::<()>() != data.len() {
            return Err(ErrorNum::EINVAL);
        }
        match op {
            IOCtlOp::Shutdown => {
                // TODO: write modified context information into nvm, then reboot. Maybe asm code.
                self.reboot();
                // The modified context will take us here, and it WILL return.
                return Ok(Vec::new())
            },
        }
    }

    fn handle_int(&self) -> Result<(), crate::utils::ErrorNum> {
        Err(ErrorNum::EINVAL)
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync> {
        self
    }

    fn as_driver<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn Driver> {
        self
    }

    fn as_int_controller<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::device::device_manager::IntController>, crate::utils::ErrorNum> {
        Err(ErrorNum::ENOTINTC)
    }

    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        Err(ErrorNum::EPERM)
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, ErrorNum> {
        Err(ErrorNum::EPERM)
    }
}
