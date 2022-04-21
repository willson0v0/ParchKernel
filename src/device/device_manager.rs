use core::any::Any;
use core::fmt::Debug;
use alloc::{boxed::Box, collections::BTreeMap, string::String, sync::Arc, vec::Vec};
use fdt_rs::{base::{DevTree, DevTreeNode}, prelude::FallibleIterator};
use lazy_static::*;
use crate::utils::{ErrorNum, SpinRWLock, UUID};

lazy_static!{
    pub static ref DEVICE_MANAGER: SpinRWLock<DeviceManager> = SpinRWLock::new(DeviceManager{
        list: BTreeMap::new(),
    });
}

pub trait Driver: Send + Sync + Debug {
    fn new(node: DevTreeNode) -> Result<Self, ErrorNum> where Self: Sized;
    fn initialize(&self) -> Result<(), ErrorNum>;
    fn terminate(&self);
    fn ioctl(&self, op: usize, data: Box<dyn Any>) -> Result<Box<dyn Any>, ErrorNum>;
    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
    fn as_driver<'a>(self: Arc<Self>) -> Arc<dyn Driver>;
}

pub struct DeviceManager {
    list: BTreeMap<UUID, Arc<dyn Driver>>,
}

impl DeviceManager {
    pub fn register_by_dtb(&mut self, device_tree: DevTree) {
        let mut node_iter = device_tree.nodes();
        while let Some(node) = node_iter.next().unwrap() {
            let node_name = node.name().unwrap();
            debug!("Initializing device: {}", node_name);

            let result: Result<Arc<dyn Driver>, ErrorNum> = match node_name {
                "dummy" => Dummy::new(node).map(|d| Arc::new(d).as_driver()),
                unknown => {
                    warning!("unrecognized device name {}", unknown);
                    Err(ErrorNum::ENOSYS)
                }
            };

            if result.is_err() {
                warning!("Device named {} failed to initialize", node_name);
                continue;
            } else {
                let uuid = UUID::new();
                info!("Registering device {} with UUID {}", node_name, uuid);
                self.list.insert(uuid, result.unwrap());
            }
        }
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

#[derive(Debug)]
struct Dummy();

impl Driver for Dummy {
    fn new(node: DevTreeNode) -> Result<Self, ErrorNum> where Self: Sized {
        Err(ErrorNum::ENOSYS)
    }

    fn initialize(&self) -> Result<(), ErrorNum> {
        todo!()
    }

    fn terminate(&self) {
        todo!()
    }

    fn ioctl(&self, op: usize, d: Box<dyn Any>) -> Result<Box<dyn Any>, ErrorNum> {
        struct Test {
            a: u8,
            b: u32
        };

        let data = d.downcast::<Test>().unwrap();
        let a = data.a;

        Ok(Box::new(Test{a: 0, b: 1}))
    }

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        todo!()
    }

    fn as_driver<'a>(self: Arc<Self>) -> Arc<dyn Driver> {
        todo!()
    }
}
