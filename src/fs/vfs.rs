use alloc::{sync::Arc, string::String, vec::Vec};
use alloc::collections::VecDeque;
use bitflags::*;
use super::{File, DirFile, RegularFile, LinkFile};
use crate::utils::ErrorNum;

bitflags! {
    /// fs flags
    pub struct OpenMode: u64 {
        const READ      = 1 << 0;
        const WRITE     = 1 << 1;
        const CREATE    = 1 << 2;
        const SYS       = 1 << 3;   // special access: opened by kernel
        const NO_FOLLOW = 1 << 5;   // do not follow symbolic link
    }
}

pub trait VirtualFileSystem : Send + Sync {
    fn open(&self, path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum>;
    fn mkdir(&self, path: &Path) -> Result<Arc<dyn DirFile>, ErrorNum>;
    fn mkfile(&self, path: &Path) -> Result<Arc<dyn RegularFile>, ErrorNum>;
    fn remove(&self, path: &Path) -> Result<(), ErrorNum>;
    fn link(&self, dest: Arc<dyn File>, link_file: &Path) -> Result<Arc<dyn File>, ErrorNum>;
    fn sym_link(&self, abs_src: &Path, rel_dst: &Path) -> Result<Arc<dyn LinkFile>, ErrorNum>;
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    components  : Vec<String>
}

impl Path {
    pub fn new(path: String) -> Result<Self, ErrorNum> {
        let mut list: VecDeque<String> = path.split('/').map(|s| String::from(s)).collect();
        if path.starts_with('/') {
            list.pop_front();
        }
        if path.ends_with('/') {
            list.pop_back();
        }
        for c in &list {
            if c.is_empty() && list.len() != 1 {
                return Err(ErrorNum::ENOENT);
            }
            // TODO: check illegal sequence?
        }
        Ok(
            Self {
                components: list.into()
            }
        )
    }

    pub fn is_root(&self) -> bool {
        return self.components.len() == 0;
    }

    pub fn root() -> Self{
        "/".into()
    }

    pub fn starts_with(&self, prefix: &Path) -> bool {
        for (this_i, pre_i) in self.components.iter().zip(prefix.components.iter()) {
            if this_i != pre_i {
                return false;
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        if self.is_root() {
            return 0
        } else {
            self.components.len()
        }
    }

    pub fn without_prefix(&self, prefix: &Path) -> Self {
        assert!(self.starts_with(prefix), "not prefix");
        Self {
            components: Vec::from(&self.components[prefix.len()..])
        }
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self::new(String::from(s)).unwrap()
    }
}