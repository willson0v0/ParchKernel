use bitflags::*;

use crate::{mem::SegmentFlags, fs::Dirent};

bitflags! {
    /// struct for MMAP prot
    pub struct MMAPProt: usize {
        const READ      = 1;
        const WRITE     = 2;
        const EXEC      = 4;
    }
}

impl Into<SegmentFlags> for MMAPProt {
    fn into(self) -> SegmentFlags {
        SegmentFlags::from_bits_truncate(self.bits << 1)
    }
}

bitflags! {
    /// struct for MMAP flag
    pub struct MMAPFlag: usize {
        const FIXED       = 0x10;
        const ANONYMOUS   = 0x20;
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallDirent {
    pub inode: u32,
    pub f_type: u16,
    pub name: [u8; 122]
}
static_assertions::assert_eq_size!(SyscallDirent, [u8; 128]);

impl From<Dirent> for SyscallDirent {
    fn from(src: Dirent) -> Self {
        let mut res = Self {
            inode: src.inode,
            f_type: src.f_type as u16,
            name: [0; 122],
        };
        let name_bytes = src.f_name.as_bytes();
        res.name[0..name_bytes.len()].copy_from_slice(name_bytes);
        res
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SyscallStat {
    pub persistant_usage: usize,
    pub runtime_usage: usize,
    pub kernel_usage: usize,
    pub total_available: usize,
}