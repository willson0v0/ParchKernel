use core::mem::size_of;

use super::{BlockNo, INodeNo};
use crate::config::PAGE_SIZE;

pub const BAD_INODE: INodeNo = 0;
pub const BAD_BLOCK: BlockNo = 0;
pub const ROOT_INODE: INodeNo = 1;
pub const DIRECT_BLOCK_COUNT: usize = 16;

pub const DENTRY_NAME_LEN: usize = 118;
pub const BLOCKNO_PER_BLK: usize = BLK_SIZE / size_of::<BlockNo>();


pub const BLK_SIZE: usize = PAGE_SIZE;
pub const INODE_SIZE: usize = 256;
pub const DENTRY_SIZE: usize = 128;