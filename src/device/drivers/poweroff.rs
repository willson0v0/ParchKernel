use alloc::{boxed::Box, sync::Arc};

use crate::{device::{device_manager::Driver, device_tree::DTBPropertyValue}, mem::PhysAddr, utils::{RWLock, UUID}};
use core::fmt::Debug;
use crate::utils::ErrorNum;

/// This is a generic poweroff dirver using syscon to map the poweroff register.
/// 32 bit access only
pub struct PowerOff {
    syscon_reg: PhysAddr,
    shutdown_magic: u32
}

impl Debug for PowerOff {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Power off driver, syscon reg @ {:?}", self.syscon_reg)
    }
}

enum_with_tryfrom_usize!{
    #[repr(usize)]
    pub enum IOCtlOp {
        Shutdown = 1,
    }
}

impl PowerOff {
    pub fn shutdown(&self) {
        unsafe {
            self.syscon_reg.write_volatile(&self.shutdown_magic)
        }
    }
}

impl Driver for PowerOff {
    fn new(dev_tree: crate::device::DeviceTree) -> Result<alloc::vec::Vec<(crate::utils::UUID, alloc::sync::Arc<dyn Driver>)>, crate::utils::ErrorNum> where Self: Sized {
        match dev_tree.serach_compatible("syscon-poweroff")?.as_slice() {
            [node_guard] => {
                let uuid = UUID::new();
                let node = node_guard.acquire_r();
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
                verbose!("Creating driver instance for poweroff, syscon reg {:?}, uuid {}, shutdown magic {}", syscon_reg, uuid, shutdown_magic);
                // sanity check
                let res = PowerOff {
                    syscon_reg,
                    shutdown_magic,
                };
                return Ok(vec![(uuid, Arc::new(res))]);
            },
            _ => panic!("No poweroff or multiple poweroff in dev_tree")
        };
    }

    fn initialize(&self) -> Result<(), crate::utils::ErrorNum> {
        Ok(())
    }

    fn terminate(&self) {
        
    }

    fn ioctl(&self, op: usize, data: alloc::boxed::Box<dyn core::any::Any>) -> Result<alloc::boxed::Box<dyn core::any::Any>, crate::utils::ErrorNum> {
        let op: IOCtlOp = op.try_into()?;
        let _sanity: () = *data.downcast().unwrap();
        match op {
            IOCtlOp::Shutdown => {
                // TODO: write modified context information into nvm, then shutdown. Maybe asm code.
                self.shutdown();
                // The modified context will take us here, and it WILL return.
                return Ok(Box::new(()))
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
}
