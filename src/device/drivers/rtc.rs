use crate::{device::device_manager::Driver, mem::PhysAddr};
use crate::utils::ErrorNum;
use core::fmt::Debug;

pub struct RTC {
    addr: PhysAddr
}

impl Debug for RTC {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "RTC @ {:?}", self.addr)
    }
}

impl Driver for RTC {
    fn new(node: fdt_rs::base::DevTreeNode) -> Result<Self, ErrorNum> where Self: Sized {
        todo!()
    }

    fn initialize(&self) -> Result<(), ErrorNum> {
        todo!()
    }

    fn terminate(&self) {
        todo!()
    }

    fn ioctl(&self, op: usize, data: alloc::boxed::Box<dyn core::any::Any>) -> Result<alloc::boxed::Box<dyn core::any::Any>, ErrorNum> {
        todo!()
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync> {
        todo!()
    }

    fn as_driver<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn Driver> {
        todo!()
    }
}