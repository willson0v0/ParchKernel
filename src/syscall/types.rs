use bitflags::*;

use crate::mem::SegmentFlags;

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