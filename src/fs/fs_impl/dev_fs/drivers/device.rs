use core::{any::{Any}, sync::atomic::{Ordering}};
use alloc::{borrow::ToOwned, boxed::Box, collections::BTreeMap, string::{String, ToString}, sync::Arc, vec::Vec};
use fdt_rs::{base::{DevTreeNode}};
use crate::{fs::CharFile, utils::{UUID, ErrorNum, RWLock, SpinRWLock, RWLockReadGuard, RWLockWriteGuard}};
use lazy_static::*;

lazy_static!{
    pub static ref DEVICE_MANAGER: DeviceManager = DeviceManager::new();
}

pub trait Device: Send + Sync {
    fn new(dtb_node: DevTreeNode) -> Result<Arc<dyn Device>, ErrorNum> where Self: Sized;
    fn name(&self) -> String;
    fn dev_file(&self) -> Arc<dyn CharFile>;
    fn uuid(&self) -> UUID;
    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn Any + Send + Sync + 'a> where Self: 'a;
    fn ioctl(&self, arg: usize) -> Result<usize, ErrorNum>;
}

pub fn as_concrete<T>(dynamic: Arc<dyn Device>) -> Result<Arc<T>, ErrorNum> where T: Device + 'static {
    Arc::downcast::<T>(dynamic.as_any()).map_err(|_| ErrorNum::ENODEV)
}

pub struct DeviceManager(SpinRWLock<DeviceManagerInner>);

pub struct DeviceManagerInner {
    dev_list: BTreeMap<String, Arc<dyn Device>>,
}

impl DeviceManager {
    pub fn new() -> Self {
        Self(SpinRWLock::new(DeviceManagerInner::new()))
    }

    pub fn read_lock(&self) -> RWLockReadGuard<DeviceManagerInner> {
        self.0.acquire_r()
    }

    pub fn write_lock(&self) -> RWLockWriteGuard<DeviceManagerInner> {
        self.0.acquire_w()
    }
}

impl DeviceManagerInner {
    pub fn new() -> Self {
        Self {
            dev_list: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, device: Arc<dyn Device>,) -> Result<(), ErrorNum> {
        let name = device.name();
        if self.dev_list.insert(name.clone(), device).is_some() {
            warning!("A device named {} already exists", name);
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<(), ErrorNum> {
        if self.dev_list.remove(name).is_some() {
            Ok(())
        } else {
            Err(ErrorNum::ENODEV)
        }
    }

    pub fn get(&self, name: &str) -> Result<Arc<dyn Device>, ErrorNum> {
        for (_, dev) in self.dev_list.iter() {
            if dev.name() == name {
                return Ok(dev.clone());
            }
        }
        Err(ErrorNum::ENODEV)
    }
}

pub struct DummyDev{}
AddCounter!(DummyDev);

impl Device for DummyDev {
    fn new(dtb_node: DevTreeNode) -> Result<Arc<dyn Device>, ErrorNum> {
        Self::COUNT.fetch_add(1, Ordering::SeqCst);
        Ok(Arc::new(Self{}))
    }

    fn name(&self) -> String {
        todo!()
    }

    fn dev_file(&self) -> Arc<dyn CharFile> {
        todo!()
    }

    fn uuid(&self) -> UUID {
        todo!()
    }

    fn ioctl(&self, op: usize) -> Result<usize, ErrorNum> {
        todo!()
    }

    fn as_any<'a>(self: Arc<Self>) -> Arc<dyn Any + Send + Sync + 'a> where Self: 'a {
        self
    }
}

impl Drop for DummyDev {
    fn drop(&mut self) {
        Self::COUNT.fetch_sub(1, Ordering::SeqCst);
    }
}