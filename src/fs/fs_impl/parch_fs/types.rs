use super::{BAD_BLOCK, BAD_INODE, ROOT_INODE, BLK_SIZE, DIRECT_BLOCK_COUNT, INODE_SIZE, DENTRY_NAME_LEN, DENTRY_SIZE};

use static_assertions::*;

pub type INodeNo = u32;
pub type BlockNo = u32;

#[repr(u16)]
pub enum Perm {
    OwnerR = 0400,
    OwnerW = 0200,
    OwnerX = 0100,
    GroupR = 0040,
    GroupW = 0020,
    GroupX = 0010,
    OtherR = 0004,
    OtherW = 0002,
    OtherX = 0001,
}

#[repr(u16)]
pub enum Type {
    SOCKET  = 0001,
    LINK    = 0002,
    REGULAR = 0004,
    BLOCK   = 0010,
    DIR     = 0020,
    CHAR    = 0040,
    FIFO    = 0100,    
}

#[repr(C)]
pub struct INode {
    permission          : Perm,
    f_type              : Type,
    uid                 : u32,
    gid                 : u32,
    f_size              : u32,
    access_time         : u64,
    change_time         : u64,
    create_time         : u64,
    flags               : u32,
    hard_link_count     : u32,
    direct_blk_no       : [BlockNo; DIRECT_BLOCK_COUNT],
    indirect_blk        : BlockNo,
    indirect_blk2       : BlockNo,
    reserved            : [u8; 136]
}

assert_eq_size!(INode, [u8; INODE_SIZE]);

#[repr(C)]
pub struct DEntry {
    inode       : INodeNo,
    permission  : Perm,
    f_type      : Type,
    name_len    : u16,
    f_name      : [u8; DENTRY_NAME_LEN]
}

assert_eq_size!(DEntry, [u8; DENTRY_SIZE]);