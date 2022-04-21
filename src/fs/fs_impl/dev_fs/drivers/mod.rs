mod goldfish_rtc;
mod uart;
mod power;
mod device;
mod virtio;
pub use goldfish_rtc::GoldFishRTC;
pub use device::DEVICE_MANAGER;

use fdt_rs::prelude::*;
use fdt_rs::base::*;
use device::Device;

use crate::utils::ErrorNum;

pub fn init() {
    info!("initializeing drivers");
    let device_tree = unsafe {
        extern "C" {
            fn device_tree_blob();
            fn device_tree_blob_end();
        }
        let dtb_buf = core::slice::from_raw_parts(device_tree_blob as usize as *const u8, device_tree_blob_end as usize - device_tree_blob as usize);
        let dtb_size = DevTree::read_totalsize(dtb_buf).unwrap();
        DevTree::new(&dtb_buf[..dtb_size]).unwrap()
    };
    let mut manager_guard = DEVICE_MANAGER.write_lock();
    let mut node_iter = device_tree.nodes();
    while let Some(node) = node_iter.next().unwrap() {
        let node_name = node.name().unwrap();
        debug!("Initializing device: {}", node_name);

        let result: Result<alloc::sync::Arc<dyn Device>, ErrorNum> = match node_name {
            "dummy" => device::DummyDev::new(node),
            unknown => {
                warning!("unrecognized device name {}", unknown);
                Err(ErrorNum::ENOSYS)
            }
        };

        if result.is_err() {
            continue;
        }

        match manager_guard.register(result.unwrap()) {
            Ok(_) => verbose!("Successfully registered {}", node_name),
            Err(error_num) => warning!("Incompatible device {}, return {:?}", node_name, error_num),
        }
    }
}