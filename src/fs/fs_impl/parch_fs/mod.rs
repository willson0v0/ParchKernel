mod types;
mod fs;
mod config;
mod base;

pub use config::*;
pub use types::*;

use lazy_static::*;

use self::fs::ParchFS;

lazy_static!{
    pub static ref PARCH_FS: alloc::sync::Arc<ParchFS> = alloc::sync::Arc::new(ParchFS::new());
}

pub use base::PFSBase;