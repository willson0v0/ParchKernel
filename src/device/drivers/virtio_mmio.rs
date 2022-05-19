use crate::device::Driver;

#[derive(Debug)]
pub struct VirtIO {}

impl Driver for VirtIO {
    fn new(dev_tree: crate::device::DeviceTree) -> Result<alloc::vec::Vec<(crate::utils::UUID, alloc::sync::Arc<dyn Driver>)>, crate::utils::ErrorNum> where Self: Sized {
        todo!()
    }

    fn write(&self, data: alloc::vec::Vec::<u8>) -> Result<usize, crate::utils::ErrorNum> {
        todo!()
    }

    fn read(&self, length: usize) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
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

    fn as_int_controller<'a>(self: alloc::sync::Arc<Self>) -> Result<alloc::sync::Arc<dyn crate::device::device_manager::IntController>, crate::utils::ErrorNum> {
        todo!()
    }
}