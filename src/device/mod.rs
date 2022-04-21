mod device_manager;
pub mod drivers;
mod device_tree;

pub use device_manager::DEVICE_MANAGER;
pub use device_tree::{
    DTBNode,
    DeviceTree
};

pub fn init() {
    
}