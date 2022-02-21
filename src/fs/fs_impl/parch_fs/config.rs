use core::mem::size_of;
use lazy_static::*;

use super::{BlockNo, INodeNo};
use crate::{config::PAGE_SIZE, mem::VirtAddr};

pub const BAD_INODE         : INodeNo = INodeNo(0);
pub const BAD_BLOCK         : BlockNo = BlockNo(0);
pub const ROOT_INODE        : INodeNo = INodeNo(1);
pub const DIRECT_BLOCK_COUNT: usize = 16;

pub const DENTRY_NAME_LEN: usize = 118;
pub const BLOCKNO_PER_BLK: usize = BLK_SIZE / size_of::<BlockNo>();


pub const BLK_SIZE: usize = PAGE_SIZE;
pub const INODE_SIZE: usize = 256;
pub const DENTRY_SIZE: usize = 128;
pub const SUPERBLOCK_SIZE: usize = PAGE_SIZE;

lazy_static! {
    pub static ref SUPERBLOCK_START: VirtAddr = {
        extern "C" {
            fn ereserve();
        }
        VirtAddr::from(ereserve as usize - SUPERBLOCK_SIZE)
    };
}