use core::fmt::{Debug, Formatter};
use alloc::string::ToString;
use alloc::{sync::Arc, string::String, vec::Vec};
use alloc::collections::VecDeque;
use bitflags::*;
use super::{File, DirFile, RegularFile, LinkFile};
use crate::mem::SegmentFlags;
use crate::utils::ErrorNum;

bitflags! {
    /// fs flags
    pub struct OpenMode: u64 {
        const READ      = 1 << 0;
        const WRITE     = 1 << 1;
        const CREATE    = 1 << 2;
        const EXEC      = 1 << 3;
        const SYS       = 1 << 4;   // special access: opened by kernel
        const NO_FOLLOW = 1 << 5;   // do not follow symbolic link
    }
}

impl Into<SegmentFlags> for OpenMode {
    fn into(self) -> SegmentFlags {
        if self.contains(OpenMode::SYS) {
            return SegmentFlags::R | SegmentFlags::W | SegmentFlags::X;
        }
        let mut res = SegmentFlags::U;
        if self.contains(OpenMode::READ) {
            res |= SegmentFlags::R;
        }
        if self.contains(OpenMode::WRITE) {
            res |= SegmentFlags::W;
        }
        if self.contains(OpenMode::EXEC) {
            res |= SegmentFlags::X;
        }
        res
    }
}

pub trait VirtualFileSystem : Send + Sync + Debug {
    fn open(&self, path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum>;
    fn mkdir(&self, path: &Path) -> Result<Arc<dyn DirFile>, ErrorNum>;
    fn mkfile(&self, path: &Path) -> Result<Arc<dyn RegularFile>, ErrorNum>;
    fn remove(&self, path: &Path) -> Result<(), ErrorNum>;
    fn link(&self, dest: Arc<dyn File>, link_file: &Path) -> Result<Arc<dyn File>, ErrorNum>;
    fn sym_link(&self, abs_src: &Path, rel_dst: &Path) -> Result<Arc<dyn LinkFile>, ErrorNum>;
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    pub components  : Vec<String>
}

impl Debug for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        for p in &self.components {
            f.write_fmt(format_args!("/{}", p))?;
        }
        Ok(())
    }
}

impl Path {
    pub fn new_s(path: String) -> Result<Self, ErrorNum> {
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

    pub fn new(path: &str) -> Result<Self, ErrorNum> {
        Self::new_s(path.to_string())
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

    pub fn append(&self, comp: String) -> Result<Path, ErrorNum> {
        if comp.contains('/') {return Err(ErrorNum::ENOENT);}
        let mut components = self.components.clone();
        components.push(comp);
        Ok(Self {
            components
        })
    }

    pub fn strip_head(&self) -> Self {
        Self {
            components: self.components[1..].to_vec()
        }
    }

    pub fn strip_tail(&self) -> Self {
        Self {
            components: self.components[..self.components.len() - 1].to_vec()
        }
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self::new(s).unwrap()
    }
}