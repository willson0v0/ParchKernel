use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use crate::utils::{SpinMutex, ErrorNum};
use super::{Path, VirtualFileSystem, File, vfs::OpenMode};

struct MountManager{
    inner: SpinMutex<MountManagerInner>
}

struct MountManagerInner {
    fs: BTreeMap<Path, Arc<dyn VirtualFileSystem>>
}

impl MountManagerInner {
    pub fn new(root_fs: Arc<dyn VirtualFileSystem>) -> Self {
        let mut res = Self {
            fs: BTreeMap::new()
        };
        res.mount("/".into(), root_fs).unwrap();
        res
    }

    pub fn mount(&mut self, path: Path, vfs: Arc<dyn VirtualFileSystem>) -> Result<(), ErrorNum> {
        if self.fs.contains_key(&path) {
            Err(ErrorNum::EADDRINUSE)
        } else {
            assert!(self.fs.insert(path, vfs).is_none(), "mount on used mounting point");
            Ok(())
        }
    }

    pub fn umount(&mut self, path: Path, force: bool) -> Result<Arc<dyn VirtualFileSystem>, ErrorNum> {
        if self.fs.contains_key(&path) {
            if force {
                Ok(self.fs.remove(&path).unwrap())
            } else {
                let to_remove = self.fs.get(&path).unwrap().clone();
                // strong fs reference in to_remove and in self.fs
                // weak fs reference in i dunno maybe files? this varies between fs
                if Arc::strong_count(&to_remove) == 2 && Arc::weak_count(&to_remove) == 0 {
                    Ok(self.fs.remove(&path).unwrap())
                } else {
                    Err(ErrorNum::EBUSY)
                }
            }
        } else {
            Err(ErrorNum::ENOENT)
        }
    }

    // This will always success for we mount rootfs at /
    pub fn parse(&self, path: &Path) -> (Arc<dyn VirtualFileSystem>, Path) {
        let mut max_match = 0;
        let mut res = None;
        let mut prefix = None;
        for (mount_point, fs) in &self.fs {
            if path.starts_with(mount_point) {
                if mount_point.len() >= max_match {
                    max_match = mount_point.len();
                    res = Some(fs.clone());
                    prefix = Some(path.without_prefix(mount_point));
                }
            }
        }
        return (res.unwrap(), prefix.unwrap());
    }

    pub fn open(&self, path: &Path, mode: OpenMode) -> Result<Arc<dyn File>, ErrorNum> {
        let (fs, rel_path) = self.parse(path);
        fs.open(&rel_path, mode)
    }
}