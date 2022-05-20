mod device_manager;
pub mod drivers;
mod device_tree;

pub use device_manager::{
    DEVICE_MANAGER,
    Driver
};
pub use device_tree::{
    DTBNode,
    DeviceTree
};

use crate::utils::RWLock;

pub fn init() {
    for (id, driver) in DEVICE_MANAGER.acquire_r().get_device_list().iter() {
        debug!("driver {:?}, uuid {}", driver, id);
    }
    milestone!("Device manager initialized.");
    DEVICE_MANAGER.acquire_r().get_dev_tree().print(crate::utils::LogLevel::Debug);
}