mod device_manager;
pub mod drivers;
mod device_tree;

pub use device_manager::DEVICE_MANAGER;
pub use device_tree::{
    DTBNode,
    DeviceTree
};

use crate::utils::RWLock;

pub fn init() {
    DEVICE_MANAGER.acquire_r(); // invoke it to trigger initialzation process
}