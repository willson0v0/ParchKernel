use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use crate::config::{MAX_LINK_RECURSE};
use crate::utils::{SpinRWLock, ErrorNum, UUID};
use super::DirFile;
use super::types::{FileType, Permission};
use super::{Path, VirtualFileSystem, File, vfs::OpenMode, LinkFile};

pub struct MountManager{
    // TODO: Change this to R/W lock
    pub inner: SpinRWLock<MountManagerInner>
}

impl MountManager {
    pub fn new(root_fs: Arc<dyn VirtualFileSystem>) -> Self {
        Self {
            inner: SpinRWLock::new(MountManagerInner::new(root_fs))
        }
    }
}

pub struct MountManagerInner {
    root_fs: Arc<dyn VirtualFileSystem>,
    fs: BTreeMap<UUID, Arc<dyn VirtualFileSystem>>,
    mount_point: BTreeMap<MountPoint, UUID>
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Copy, Clone)]
struct MountPoint {
    pub fs: UUID,
    pub inode: u32,
}

impl MountPoint {
    pub fn match_dir(&self, dir: Arc<dyn DirFile>) -> Result<bool, ErrorNum> {
        Ok(*self == Self::from_dir(dir)?)
    }

    pub fn from_dir(dir: Arc<dyn DirFile>) -> Result<Self, ErrorNum> {
        let stat = dir.stat()?;
        Ok(Self {
            fs: stat.fs.upgrade().unwrap().get_uuid(),
            inode: stat.inode,
        })
    }
}

impl MountManagerInner {
    pub fn new(root_fs: Arc<dyn VirtualFileSystem>) -> Self {
        let mut fs = BTreeMap::new();
        fs.insert(root_fs.get_uuid(), root_fs.clone());
        Self {
            root_fs,
            fs,
            mount_point: BTreeMap::new(),
        }
    }

    pub fn open(&self, path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
        self.open_path_inner(self.root_fs.root_dir(mode)?.as_file(), path, mode, 0)
    }

    pub fn open_at(&self, src: Arc<dyn File>, path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
        self.open_path_inner(src, path, mode, 0)
    }

    fn open_path_inner(&self, mut lookup: Arc<dyn File>, path: &Path, mode: OpenMode, recurse_count: usize) -> Result<Arc<dyn File>, ErrorNum> {
        if recurse_count >= MAX_LINK_RECURSE {
            return Err(ErrorNum::EMLINK)
        }
        let mut path = path.to_owned();
        while !path.is_root() {
            verbose!("Opening {:?} -> {:?}", lookup, path);
            if let Ok(dir) = lookup.clone().as_dir() {
                let mp = MountPoint::from_dir(dir.clone())?;
                if self.mount_point.contains_key(&mp) {
                    verbose!("Following mount.");
                    lookup = self.get_fs(*self.mount_point.get(&mp).unwrap()).unwrap().root_dir(mode)?.as_file();
                } else {
                    lookup = dir.open_entry(&path.components[0], mode)?;
                    path = path.strip_head();
                }
            } else if let Ok(link) = lookup.clone().as_link() {
                if mode.contains(OpenMode::NO_FOLLOW) {
                    return Err(ErrorNum::ENOENT)
                }
                verbose!("Following link.");
                lookup = self.open_path_inner(self.root_fs.root_dir(mode)?.as_file(), &link.read_link()?, mode, recurse_count + 1)?;
            } else {
                return Err(ErrorNum::ENOENT)
            }
        }
        // mount root cannot be a link, so first check link (recursively) then check mount
        if let Ok(link) = lookup.clone().as_link() {
            if !mode.contains(OpenMode::NO_FOLLOW) {
                lookup = self.open_path_inner(self.root_fs.root_dir(mode)?.as_file(), &link.read_link()?, mode, recurse_count+1)?;
            }
        }
        if let Ok(dir) = lookup.clone().as_dir() {
            let mp = MountPoint::from_dir(dir)?;
            if self.mount_point.contains_key(&mp) {
                verbose!("Following mount.");
                lookup = self.get_fs(*self.mount_point.get(&mp).unwrap()).unwrap().root_dir(mode)?.as_file();
            }
        }
        Ok(lookup)
    }

    pub fn mount(&mut self, path: Path, vfs: Arc<dyn VirtualFileSystem>) -> Result<(), ErrorNum> {
        let stat = self.open(&path, OpenMode::SYS)?.stat()?;
        let mount_point = MountPoint{
            fs: stat.fs.upgrade().unwrap().get_uuid(),
            inode: stat.inode,
        };
        self.mount_point.insert(mount_point, vfs.get_uuid());
        self.fs.insert(vfs.get_uuid(), vfs);
        Ok(())
        // mount_vfs.mount(mount_dir, path.last(), vfs)
    }

    pub fn umount(&mut self, path: Path, _force: bool) -> Result<(), ErrorNum> {
        let mount_dir = self.open(&path, OpenMode::SYS)?.as_dir()?;
        let mp = MountPoint::from_dir(mount_dir)?;
        if self.mount_point.contains_key(&mp) {
            let fs = self.mount_point.remove(&mp).unwrap();
            self.fs.remove(&fs).unwrap();
            Ok(())
        } else {
            Err(ErrorNum::ENOENT)
        }
    }
    
    pub fn make_file(&self, path: &Path, perm: Permission, f_type: FileType) -> Result<(), ErrorNum> {
        verbose!("make file for {:?}, type {:?}", path, f_type);
        let dir = self.open(&path.strip_tail(), OpenMode::READ | OpenMode::WRITE)?.as_dir()?;
        dir.make_file(path.last().clone(), perm, f_type)?;
        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<(), ErrorNum> {
        let dir = self.open(&path.strip_tail(), OpenMode::READ | OpenMode::WRITE)?.as_dir()?;
        dir.remove_file(path.last().clone())
    }

    // hard link
    pub fn link(&self, dest: &Path, link_file: &Path) -> Result<(), ErrorNum>{
        let dest_file = self.open(dest, OpenMode::SYS)?;
        let dest_vfs = dest_file.vfs();
        let link_dir = self.open(&link_file.strip_tail(), OpenMode::READ | OpenMode::WRITE)?.as_dir()?;
        let link_vfs = link_dir.vfs();
        if Arc::ptr_eq(&dest_vfs, &link_vfs) {
            link_vfs.link(dest_file, &link_file.without_prefix(&link_vfs.mount_path()))?;
            Ok(())
        } else {
            Err(ErrorNum::EXDEV)
        }
    }

    pub fn sym_link(&self, target: &Path, link_file_path: &Path, perm: Permission) -> Result<Arc<dyn LinkFile>, ErrorNum>{
        self.make_file(link_file_path, perm, FileType::LINK)?;
        let link_file = self.open(link_file_path, OpenMode::SYS)?.as_link()?;
        if link_file.write_link(target).is_ok() {
            Ok(link_file)
        } else {
            self.remove(link_file_path).unwrap();
            Err(ErrorNum::EPERM)
        }
    }

    pub fn get_fs(&self, uuid: UUID) -> Result<Arc<dyn VirtualFileSystem>, ErrorNum> {
        self.fs.get(&uuid).cloned().ok_or(ErrorNum::ENOENT)
    }
}