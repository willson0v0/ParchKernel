use core::mem::size_of;


use super::{BlockNo, INodeNo};
use crate::{config::PAGE_SIZE};

pub const BAD_INODE         : INodeNo = INodeNo(0);
pub const BAD_BLOCK         : BlockNo = BlockNo(0);
pub const ROOT_INODE        : INodeNo = INodeNo(1);
pub const DIRECT_BLK_COUNT: usize = 16;

pub const DENTRY_NAME_LEN: usize = 118;
pub const BLOCKNO_PER_BLK: usize = BLK_SIZE / size_of::<BlockNo>();


pub const BLK_SIZE: usize = PAGE_SIZE;
pub const INODE_SIZE: usize = 256;
pub const DENTRY_SIZE: usize = 128;
pub const SUPERBLOCK_SIZE: usize = PAGE_SIZE;
pub const INODE_BITMAP_SIZE: usize = BLK_SIZE;
pub const INODE_LIST_SIZE: usize = 512 * BLK_SIZE;


pub const PFS_MAGIC: u64 = 0xBEEF_BEEF_BEEF_BEEF;
pub const PFS_MAXCAP: usize = DIRECT_BLK_COUNT * BLK_SIZE + BLOCKNO_PER_BLK * BLK_SIZE + BLK_SIZE * BLOCKNO_PER_BLK * BLOCKNO_PER_BLK;