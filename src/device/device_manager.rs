use core::any::Any;
use core::fmt::Debug;
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
// use fdt_rs::{base::{DevTree, DevTreeNode}, prelude::FallibleIterator};
use lazy_static::*;
use crate::{mem::PhysAddr, utils::{ErrorNum, SpinRWLock, UUID}};

use super::{DeviceTree, drivers::{rtc::RTC, uart::UART}};

lazy_static!{
    pub static ref DEVICE_MANAGER: SpinRWLock<DeviceManager> = SpinRWLock::new(DeviceManager{
        list: BTreeMap::new(),
        dev_tree: {
            extern "C" {
                fn device_tree_blob();
            }
            DeviceTree::parse(PhysAddr::from(device_tree_blob as usize)).unwrap()
        }
    });
}

pub trait Driver: Send + Sync + Debug {
    fn new(devtree: DeviceTree) -> Result<Vec<(UUID, Arc<dyn Driver>)>, ErrorNum> where Self: Sized;
    fn initialize(&self) -> Result<(), ErrorNum>;
    fn terminate(&self);
    fn ioctl(&self, op: usize, data: Box<dyn Any>) -> Result<Box<dyn Any>, ErrorNum>;
    fn handle_int(&self) -> Result<(), ErrorNum>;
    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn as_driver<'a>(self: Arc<Self>) -> Arc<dyn Driver>;
}

pub struct DeviceManager {
    list: BTreeMap<UUID, Arc<dyn Driver>>,
    dev_tree: DeviceTree
}

impl DeviceManager {
    pub fn register_by_dtb(&mut self, device_tree: DeviceTree) -> Result<(), ErrorNum> {
        self.list.append(&mut RTC::new(device_tree.clone())?.into_iter().collect());
        self.list.append(&mut UART::new(device_tree.clone())?.into_iter().collect());
        Ok(())
    }

    // call this after boot and register, or warm reboot
    pub fn init_all(&self) -> Result<(), ErrorNum> {
        for driver in self.list.values() {
            driver.initialize()?;
        }
        Ok(())
    }

    pub fn get_device(&self, uuid: UUID) -> Result<Arc<dyn Driver>, ErrorNum> {
        self.list.get(&uuid).cloned().ok_or(ErrorNum::ENODEV)
    }

    pub fn get_device_list(&self) -> Vec<(UUID, Arc<dyn Driver>)> {
        self.list.iter().map(|(uuid, driver)| (uuid.clone(), driver.clone())).collect()
    }
}