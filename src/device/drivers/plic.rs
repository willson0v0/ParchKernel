use alloc::sync::Arc;
use crate::utils::ErrorNum;
use crate::device::device_manager::{Driver, IntController};

#[derive(Debug)]
pub struct PLIC {

}

impl Driver for PLIC {
    fn new(devtree: crate::device::DeviceTree) -> Result<alloc::vec::Vec<(crate::utils::UUID, alloc::sync::Arc<dyn Driver>)>, crate::utils::ErrorNum> where Self: Sized {
        todo!()
    }

    fn initialize(&self) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn terminate(&self) {
        todo!()
    }

    fn ioctl(&self, op: usize, data: alloc::boxed::Box<dyn core::any::Any>) -> Result<alloc::boxed::Box<dyn core::any::Any>, crate::utils::ErrorNum> {
        todo!()
    }

    fn handle_int(&self) -> Result<(), crate::utils::ErrorNum> {
        todo!()
    }

    fn as_any<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn core::any::Any + Send + Sync> {
        todo!()
    }

    fn as_driver<'a>(self: alloc::sync::Arc<Self>) -> alloc::sync::Arc<dyn Driver> {
        todo!()
    }

    fn as_int_controller<'a>(self: Arc<Self>) -> Result<Arc<dyn crate::device::device_manager::IntController>, ErrorNum> {
        Ok(self)
    }
}

impl IntController for PLIC {
    fn clear_int(&self, int_num: u32) -> Result<(), ErrorNum> {
        todo!()
    }

    fn enable_int(&self, int_num: u32) -> Result<(), ErrorNum> {
        todo!()
    }

    fn disable_int(&self, int_num: u32) -> Result<(), ErrorNum> {
        todo!()
    }
}
