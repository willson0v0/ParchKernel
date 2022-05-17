use core::any::Any;
use core::fmt::Debug;
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
// use fdt_rs::{base::{DevTree, DevTreeNode}, prelude::FallibleIterator};
use lazy_static::*;
use crate::{mem::PhysAddr, utils::{ErrorNum, RWLock, SpinRWLock, UUID}};

use super::{DeviceTree, device_tree::DTBPropertyValue, drivers::{plic::PLIC, rtc::RTC, uart::UART}};

lazy_static!{
    pub static ref DEVICE_MANAGER: SpinRWLock<DeviceManager> = {
        extern "C" {
            fn device_tree_blob();
        }
        let dev_tree = DeviceTree::parse(PhysAddr::from(device_tree_blob as usize)).unwrap();
        SpinRWLock::new(DeviceManager{
            list: BTreeMap::new(),
            int_controller : {
                match PLIC::new(dev_tree.clone()).unwrap().as_slice() {
                    [(_uuid, driver)] => driver.clone().as_int_controller().unwrap(),
                    _ => panic!("No int controller found")
                }
            },
            dev_tree
        })
    };
}

pub enum DeviceStatus {
    Uninitialized,
    Running,
    Terminated,
    Custom(Box<dyn Any + Send + Sync>)
}

pub trait Driver: Send + Sync + Debug {
    fn new(dev_tree: DeviceTree) -> Result<Vec<(UUID, Arc<dyn Driver>)>, ErrorNum> where Self: Sized;
    fn initialize(&self) -> Result<(), ErrorNum>;
    fn terminate(&self);
    fn ioctl(&self, op: usize, data: Box<dyn Any>) -> Result<Box<dyn Any>, ErrorNum>;
    fn handle_int(&self) -> Result<(), ErrorNum>;
    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn as_driver<'a>(self: Arc<Self>) -> Arc<dyn Driver>;
    fn as_int_controller<'a>(self: Arc<Self>) -> Result<Arc<dyn IntController>, ErrorNum>;
}

pub trait IntController: Driver {
    fn clear_int(&self, int_num: u32) -> Result<(), ErrorNum>;
    fn claim_int(&self) -> Result<u32, ErrorNum>;
}

pub struct DeviceManager {
    list: BTreeMap<UUID, Arc<dyn Driver>>,
    /// there will be only ONE interrupt gateway(PLIC) in risc-v spec
    int_controller: Arc<dyn IntController>,
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

    pub fn handle_interrupt(&self) -> Result<(), ErrorNum> {
        let int_id = self.int_controller.claim_int().unwrap();

        let dtb_node = self.dev_tree.search_single("interrupts", DTBPropertyValue::UInt32(int_id))?;
        let driver = self.get_device(dtb_node.acquire_r().driver)?;
        driver.handle_int()?;

        self.int_controller.clear_int(int_id)
    }
}