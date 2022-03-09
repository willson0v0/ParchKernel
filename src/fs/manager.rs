use alloc::borrow::ToOwned;
use alloc::{collections::BTreeMap};
use alloc::sync::Arc;
use crate::config::{MAX_LINK_RECURSE};
use crate::utils::{SpinRWLock, ErrorNum, UUID};
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
    fs: BTreeMap<UUID, Arc<dyn VirtualFileSystem>>
}

impl MountManagerInner {
    pub fn new(root_fs: Arc<dyn VirtualFileSystem>) -> Self {
        let mut fs = BTreeMap::new();
        fs.insert(root_fs.get_uuid(), root_fs.clone());
        Self {
            root_fs,
            fs
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
                lookup = dir.open_entry(&path.components[0], mode)?;
                path = path.strip_head();
            } else if let Ok(link) = lookup.clone().as_link() {
                if mode.contains(OpenMode::NO_FOLLOW) {
                    return Err(ErrorNum::ENOENT)
                }
                verbose!("Following link.");
                lookup = self.open_path_inner(self.root_fs.root_dir(mode)?.as_file(), &link.read_link()?, mode, recurse_count + 1)?;
            } else if let Ok(mount) = lookup.clone().as_mount() {
                verbose!("Following mount.");
                lookup = self.get_fs(mount.get_uuid())?.root_dir(mode)?.as_file();
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
        if let Ok(mount) = lookup.clone().as_mount() {
            lookup = self.get_fs(mount.get_uuid())?.root_dir(mode)?.as_file();
        }
        Ok(lookup)
    }

    pub fn mount(&mut self, path: Path, vfs: Arc<dyn VirtualFileSystem>) -> Result<(), ErrorNum> {
        let mount_dir = self.open(&path.strip_tail(), OpenMode::SYS)?.as_dir()?;
        mount_dir.register_mount(path.last(), vfs.get_uuid())?;
        self.fs.insert(vfs.get_uuid(), vfs);
        Ok(())
        // mount_vfs.mount(mount_dir, path.last(), vfs)
    }

    pub fn umount(&mut self, path: Path, _force: bool) -> Result<(), ErrorNum> {
        let mount_dir = self.open(&path.strip_tail(), OpenMode::SYS)?.as_dir()?;
        self.fs.remove(&mount_dir.register_umount(path.last())?).ok_or(ErrorNum::ENOENT)?;
        Ok(())
    }
    
    pub fn make_file(&self, path: &Path, perm: Permission, f_type: FileType) -> Result<(), ErrorNum> {
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
            Err(ErrorNum::ELINKCROSSFS)
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