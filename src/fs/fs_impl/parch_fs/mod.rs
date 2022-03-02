mod types;
mod fs;
mod config;
mod base;

pub use config::*;
pub use types::*;

use lazy_static::*;

use crate::fs::Path;

use self::fs::ParchFS;

lazy_static!{
    pub static ref PARCH_FS: alloc::sync::Arc<ParchFS> = {
        let root_path: Path = "/".into();
        let res = alloc::sync::Arc::new(ParchFS::new(root_path.clone()));
        milestone!("ParchFS initialized on {:?}", root_path);
        res
    };
}

pub use base::PFSBase;