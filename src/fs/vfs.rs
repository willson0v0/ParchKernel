use core::fmt::{Debug, Formatter};

use alloc::borrow::ToOwned;
use alloc::string::ToString;
use alloc::{sync::Arc, string::String, vec::Vec};
use alloc::collections::VecDeque;
use bitflags::*;
use super::{File, LinkFile};
use crate::mem::SegmentFlags;
use crate::utils::ErrorNum;

bitflags! {
    /// fs flags
    pub struct OpenMode: usize {
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
    fn mkdir(&self, path: &Path) -> Result<(), ErrorNum>;
    fn mkfile(&self, path: &Path) -> Result<(), ErrorNum>;
    // TODO: chmod
    fn remove(&self, path: &Path) -> Result<(), ErrorNum>;
    fn link(&self, dest: Arc<dyn File>, link_file: &Path) -> Result<Arc<dyn File>, ErrorNum>;
    fn sym_link(&self, abs_src: &Path, rel_dst: &Path) -> Result<Arc<dyn LinkFile>, ErrorNum>;
    fn mount_path(&self) -> Path;
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Path {
    pub components  : Vec<String>
}

impl Debug for Path {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "/")?;
        for p in &self.components {
            write!(f, "{}", p)?;
            if self.components.last().unwrap() != p {
                write!(f, "/")?;
            }
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
        let res: Path = "/".into();
        assert!(res.len() == 0);
        res
    }

    pub fn starts_with(&self, prefix: &Path) -> bool {
        if prefix.len() == 0 {return true;}
        if prefix.len() > self.len() {return false;}
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
        if self.is_root() {panic!("already root")}
        Self {
            components: self.components[1..].to_vec()
        }
    }

    pub fn strip_tail(&self) -> Self {
        if self.is_root() {panic!("already root")}
        Self {
            components: self.components[..self.components.len() - 1].to_vec()
        }
    }

    pub fn last(&self) -> String {
        if self.is_root() {panic!("is_root")}
        return self.components[self.len() - 1].clone();
    }

    pub fn concat(&self, rhs: &Path) -> Self {
        let mut components = self.components.clone();
        components.append(&mut rhs.components.clone());
        Self {
            components
        }
    }

    pub fn reduce(&mut self) {
        let mut new_component = Vec::new();
        for c in self.components.clone().into_iter() {
            if c == ".." && new_component.len() != 0{
                new_component.pop();
            } else  if c != "." {
                new_component.push(c);
            }
        }
        self.components = new_component;
    }
}

impl From<&str> for Path {
    fn from(s: &str) -> Self {
        Self::new(s).unwrap()
    }
}

impl From<String> for Path {
    fn from(s: String) -> Self {
        Self::new_s(s).unwrap()
    }
}