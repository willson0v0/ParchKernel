use crate::{fs::{vfs::OpenMode, fs_impl::parch_fs::{BAD_BLOCK, BLOCKNO_PER_BLK, PFS_MAXCAP}, Path, types::FileType, Cursor}, mem::{PageGuard, VirtPageNum, claim_fs_page, VirtAddr}, utils::{ErrorNum, Mutex, MutexGuard, time::get_real_time_epoch}};
use super::{DIRECT_BLK_COUNT, BLK_SIZE, fs::{ParchFS, ParchFSInner}, BlockNo, INodeNo, PFSINode};


use core::cmp::min;
use alloc::{sync::{Weak, Arc}};
use alloc::vec::Vec;


pub struct PFSBase {
    pub inode_no: INodeNo,
    pub open_mode: OpenMode,
    pub mmap_start: Option<VirtPageNum>,
    pub fs: Weak<ParchFS>,
    pub path: Path
}

impl PFSBase {
    pub fn new(inode_no: INodeNo, path: Path, open_mode: OpenMode, fs: Weak<ParchFS>) -> Result<Self, ErrorNum> {
        Ok(Self {
            inode_no,
            mmap_start: None,
            open_mode,
            fs,
            path
        })
    }

    pub fn get_blockno_locked(&self, offset: usize, create: bool, fs_inner: &mut MutexGuard<ParchFSInner>, inode: &mut MutexGuard<&mut PFSINode>) -> Result<BlockNo, ErrorNum> {
        let mut offset: usize = offset as usize;
        if offset >= PFS_MAXCAP {
            return Err(ErrorNum::EOOR);
        }
        if create && offset > inode.f_size {
            inode.f_size = offset;
        } else if offset >= inode.f_size {
            return Err(ErrorNum::EOOR);
        }
        if create {
            for i in 0..min(DIRECT_BLK_COUNT, offset / BLK_SIZE + 1) {
                if inode.direct_blk_no[i] == BAD_BLOCK {
                    inode.direct_blk_no[i] = fs_inner.alloc_blk();
                }
            }
        }
        if offset < BLK_SIZE * DIRECT_BLK_COUNT {
            let res = inode.direct_blk_no[offset / BLK_SIZE];
            assert!(res != BAD_BLOCK, "Malformed fs");
            return Ok(res);
        }
        offset -= BLK_SIZE * DIRECT_BLK_COUNT;

        // indirect 1
        if create {
            if inode.indirect_blk == BAD_BLOCK {
                inode.indirect_blk = fs_inner.alloc_blk();
            }

            let indirect_blk_pa = ParchFS::blockno_2_pa(inode.indirect_blk);
            let blocks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe {
                indirect_blk_pa.instantiate_volatile()
            };
            
            for i in 0..min(BLOCKNO_PER_BLK, offset / BLK_SIZE + 1) {
                if blocks[i] == BAD_BLOCK {
                    blocks[i] = fs_inner.alloc_blk();
                }
            }
        }
        assert!(inode.indirect_blk != BAD_BLOCK, "malformed fs");
        if offset < BLK_SIZE * BLOCKNO_PER_BLK {
            let indirect_blk_pa = ParchFS::blockno_2_pa(inode.indirect_blk);
            let blocks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe {
                indirect_blk_pa.instantiate_volatile()
            };
            let res = blocks[offset / BLK_SIZE];
            assert!(res != BAD_BLOCK, "malformed fs");
            return Ok(res);
        }
        offset -= BLK_SIZE * BLOCKNO_PER_BLK;

        // indirect 2
        if create {
            if inode.indirect_blk2 == BAD_BLOCK {
                inode.indirect_blk2 = fs_inner.alloc_blk();
            }

            let lv1_indirect_blk_pa = ParchFS::blockno_2_pa(inode.indirect_blk2);
            let lv1_indirect_blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe {
                lv1_indirect_blk_pa.instantiate_volatile()
            };

            for i in 0..min(BLOCKNO_PER_BLK, offset / (BLOCKNO_PER_BLK * BLK_SIZE) + 1) {
                if lv1_indirect_blks[i] == BAD_BLOCK {
                    lv1_indirect_blks[i] = fs_inner.alloc_blk();
                }

                let lv2_indirect_blk_pa = ParchFS::blockno_2_pa(lv1_indirect_blks[i]);
                let lv2_indirect_blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe {
                    lv2_indirect_blk_pa.instantiate_volatile()
                };

                for j in 0..BLOCKNO_PER_BLK {
                    if lv2_indirect_blks[i] == BAD_BLOCK {
                        lv2_indirect_blks[i] = fs_inner.alloc_blk();
                    }

                    let lv2_cap = i * BLOCKNO_PER_BLK * BLK_SIZE + j * BLK_SIZE;
                    if lv2_cap > offset {
                        break;
                    }
                }
            }
        }
        assert!(inode.indirect_blk2 != BAD_BLOCK, "Malformed fs");
        let blk_offset = offset / BLK_SIZE;
        let lv1_indirect_blk_pa = ParchFS::blockno_2_pa(inode.indirect_blk2);
        let lv1_indirect_blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe {
            lv1_indirect_blk_pa.instantiate_volatile()
        };
        let lv1_blkno = lv1_indirect_blks[blk_offset / BLOCKNO_PER_BLK];
        
        assert!(lv1_blkno != BAD_BLOCK, "Malformed fs");
        let lv2_indirect_blk_pa = ParchFS::blockno_2_pa(lv1_blkno);
        let lv2_indirect_blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe {
            lv2_indirect_blk_pa.instantiate_volatile()
        };
        let lv2_blkno = lv2_indirect_blks[blk_offset % BLOCKNO_PER_BLK];

        assert!(lv2_blkno != BAD_BLOCK, "Malformed fs");
        return Ok(lv2_blkno);
    }

    pub fn get_blockno(&self, offset: usize, create: bool) -> Result<BlockNo, ErrorNum> {
        let offset: usize = offset as usize;
        let fs = self.fs.clone().upgrade().unwrap();
        let mut fs_inner = fs.inner.acquire();
        let inode_guard = fs_inner.get_inode(self.inode_no)?;
        let mut inode = inode_guard.acquire();
        self.get_blockno_locked(offset, create, &mut fs_inner, &mut inode)
    }

    pub fn expand(&self, offset: usize) -> Result<(), ErrorNum> {
        self.get_blockno(offset, true)?;
        Ok(())
    }

    pub fn expand_locked(&self, offset: usize, fs_inner: &mut MutexGuard<ParchFSInner>, inode: &mut MutexGuard<&mut PFSINode>) -> Result<(), ErrorNum> {
        self.get_blockno_locked(offset, true, fs_inner, inode)?;
        Ok(())
    }

    pub fn resize(&self, new_size: usize) -> Result<(), ErrorNum> {
        let new_size: usize = new_size as usize;
        let fs = self.fs.clone().upgrade().unwrap();
        let mut fs_inner = fs.inner.acquire();
        let inode_guard = fs_inner.get_inode(self.inode_no)?;
        let mut inode = inode_guard.acquire();
        self.resize_locked(new_size, &mut fs_inner, &mut inode)
    }

    pub fn resize_locked(&self,  new_size: usize, fs_inner: &mut MutexGuard<ParchFSInner>, inode: &mut MutexGuard<&mut PFSINode>) -> Result<(), ErrorNum> {
        if inode.f_size < new_size {
            return self.expand_locked(new_size, fs_inner, inode);
        }

        if inode.f_size == new_size {
            return Ok(());
        }

        if new_size == 0 {
            return Err(ErrorNum::EEMPTY);
        }

        let shrink_start = (new_size - 1) / BLK_SIZE + 1;

        if shrink_start <= DIRECT_BLK_COUNT + BLOCKNO_PER_BLK {
            // all lv2 are gone
            self.free_blockno(inode.indirect_blk2, 2, fs_inner, inode);
            inode.indirect_blk2 = BAD_BLOCK;
            if shrink_start <= DIRECT_BLK_COUNT {
                // all lv1 are gone
                self.free_blockno(inode.indirect_blk, 1, fs_inner, inode);
                inode.indirect_blk = BAD_BLOCK;
                // some lv0 are gone
                for i in shrink_start..DIRECT_BLK_COUNT {
                    self.free_blockno(inode.direct_blk_no[i], 0, fs_inner, inode);
                    inode.direct_blk_no[i] = BAD_BLOCK;
                }
            } else {
                // some lv1 are gone
                let lv1_blks_pa = ParchFS::blockno_2_pa(inode.indirect_blk);
                let lv1_blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe{lv1_blks_pa.instantiate_volatile()};
                let start = shrink_start - DIRECT_BLK_COUNT;
                for i in start..BLOCKNO_PER_BLK {
                    self.free_blockno(lv1_blks[i], 0, fs_inner, inode);
                    lv1_blks[i] = BAD_BLOCK;
                }
            }
        } else {
            // some lv2 are gone
            let lv2_blks_pa = ParchFS::blockno_2_pa(inode.indirect_blk2);
            let lv2_blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe{lv2_blks_pa.instantiate_volatile()};
            // remove lv2 entry
            let start = shrink_start - DIRECT_BLK_COUNT - BLOCKNO_PER_BLK;
            let lv2_start = (start - 1) / BLOCKNO_PER_BLK + 1;  // first lv2 blk should preserve, and remove part of lv1 blk within
            for i in lv2_start..BLOCKNO_PER_BLK {
                self.free_blockno(lv2_blks[i], 1, fs_inner, inode);
                lv2_blks[i] = BAD_BLOCK;
            }
            // remove first lv2 -> lv1 entry
            let lv1_start = start % BLOCKNO_PER_BLK;
            if lv1_start != 0 {
                let lv1_blks_pa = ParchFS::blockno_2_pa(lv2_blks[lv2_start - 1]);
                let lv1_blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe{lv1_blks_pa.instantiate_volatile()};
                for i in lv1_start..BLOCKNO_PER_BLK {
                    self.free_blockno(lv1_blks[i], 0, fs_inner, inode);
                    lv1_blks[i] = BAD_BLOCK;
                }
            }
        }

        inode.f_size = new_size;
        Ok(())
    }

    /// lvl == 0: direct
    /// lvl == 1: indirect 1
    /// lvl == 2: indirect 2
    /// must set block_no to BAD_BLOCK after calling this
    pub fn free_blockno(&self, block_no: BlockNo, lvl: usize, fs_inner: &mut MutexGuard<ParchFSInner>, inode: &mut MutexGuard<&mut PFSINode>) {
        if block_no == BAD_BLOCK {return;}
        if lvl >= 1 {
            let blks_pa = ParchFS::blockno_2_pa(block_no);
            let blks: &mut [BlockNo; BLOCKNO_PER_BLK] = unsafe{blks_pa.instantiate_volatile()};
            for i in 0..BLOCKNO_PER_BLK {
                self.free_blockno(blks[i], lvl-1, fs_inner, inode);
                blks[i] = BAD_BLOCK;
            }
        }
        fs_inner.free_blk(block_no);
    }

    pub fn f_type(&self) -> Result<FileType, ErrorNum> {
        let fs = self.fs.clone().upgrade().unwrap();
        let mut fs_inner = fs.inner.acquire();
        let inode_guard = fs_inner.get_inode(self.inode_no)?;
        let inode = inode_guard.acquire();
        Ok(inode.f_type.into())
    }
    
    // if inode was gone (deleted by other process), cannot write but can still read from remained mmap.
    pub fn write(&self, data: alloc::vec::Vec::<u8>, offset: Cursor) -> Result<(), crate::utils::ErrorNum> {
        let mut offset = offset.0;
        if data.len() == 0 {return Ok(())}
        let fs = self.fs.upgrade().unwrap();
        let mut fs_inner = fs.inner.acquire();
        let inode_guard = fs_inner.get_inode(self.inode_no)?;
        let mut inode = inode_guard.acquire();
        inode.change_time = get_real_time_epoch();
        inode.access_time = get_real_time_epoch();
        if inode.f_size < offset + data.len() {
            self.expand_locked(offset + data.len(), &mut fs_inner, &mut inode)?;
        }
        if let Some(mmap_start) = self.mmap_start {
            let start_va = VirtAddr::from(mmap_start) + offset;
            unsafe{start_va.write_data(data)};
            Ok(())
        } else {
            let length = data.len();
            let target = length + offset;
            let mut data_ptr = 0;
            while offset < target {
                let blk = self.get_blockno_locked(offset, false, &mut fs_inner, &mut inode)?;
                let pa = ParchFS::blockno_2_pa(blk);
                // offset to pa
                let dst_start = offset % BLK_SIZE;
                let dst_end = if target > offset + (BLK_SIZE - dst_start) {
                    BLK_SIZE
                } else {
                    target % BLK_SIZE
                };
                let cpy_size = dst_end - dst_start;

                let src_start = data_ptr;
                let src_end = src_start + cpy_size;

                unsafe{&(pa + dst_start).write_data(data[src_start..src_end].to_vec())};
                offset += cpy_size;
                data_ptr += cpy_size;
            }
            Ok(())
        }
    }

    pub fn read(&self, mut length: usize, offset: Cursor) -> Result<alloc::vec::Vec<u8>, crate::utils::ErrorNum> {
        let mut offset = offset.0;
        let fs = self.fs.upgrade().unwrap();
        let mut fs_inner = fs.inner.acquire();
        let inode_guard = fs_inner.get_inode(self.inode_no)?;
        let mut inode = inode_guard.acquire();
        inode.access_time = get_real_time_epoch();

        // truncate
        if inode.f_size <= offset + length {
            if offset > inode.f_size {
                length = 0;
            } else {
                length = inode.f_size - offset;
            }
        }

        if length == 0 {return Ok(Vec::new())}
        
        if let Some(mmap_start) = self.mmap_start {
            unsafe {
                Ok((VirtAddr::from(mmap_start) + offset).read_data(length))
            }
        } else {
            let _fs = self.fs.upgrade().unwrap();
            let mut result: Vec<u8> = Vec::new();
            let target = length + offset;
            while offset < target {
                let blk = self.get_blockno_locked(offset, false, &mut fs_inner, &mut inode)?;
                let pa = ParchFS::blockno_2_pa(blk);

                let cpy_start = offset % BLK_SIZE;
                let cpy_end = if target > offset + (BLK_SIZE - cpy_start) {
                    BLK_SIZE
                } else {
                    target % BLK_SIZE
                };
                let cpy_size = cpy_end - cpy_start;
                result.append(&mut unsafe{(pa + cpy_start).read_data(cpy_size).clone()});
                offset += cpy_size;
            }
            Ok(result)
        }
    }

    pub fn vfs(&self) -> Arc<dyn crate::fs::VirtualFileSystem> {
        self.fs.upgrade().unwrap()
    }

    pub fn stat(&self) -> Result<crate::fs::types::FileStat, ErrorNum> {
        let fs_guard = self.fs.upgrade().unwrap();
        let mut fs = fs_guard.inner.acquire();
        let inode_guard = fs.get_inode(self.inode_no)?;
        let inode = inode_guard.acquire();
        // let fs_mount_path ;
        Ok(crate::fs::types::FileStat { 
            open_mode: self.open_mode, 
            file_size: inode.f_size,
            path: self.path.clone(), 
            inode: self.inode_no.0, 
            fs: self.fs.clone()
        })
    }
    
    pub fn get_page(&self, offset: usize) -> Result<PageGuard, ErrorNum> {
        if offset % BLK_SIZE != 0 {
            Err(ErrorNum::ENOTALIGNED)
        } else {
            let block_no = self.get_blockno(offset, false)?;
            let block_ppn = ParchFS::blockno_2_ppn(block_no);
            Ok(claim_fs_page(block_ppn))
        }
    }
}
