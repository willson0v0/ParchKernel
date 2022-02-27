use _core::mem::size_of;
use alloc::vec::Vec;
use bitflags::*;

use core::fmt::{self, Debug, Formatter};

use crate::{utils::LogLevel, config::PAGE_SIZE, process::ProcessID};

use super::{PageGuard, PhysAddr, alloc_vm_page, types::{PhysPageNum, VirtPageNum}};

bitflags! {
    /// Pagetable entry flags, indicating privileges.
    pub struct PTEFlags: usize {
        /// valid
        const V = 1 << 0;   
        /// read enable
        const R = 1 << 1;   
        /// write enable
        const W = 1 << 2;   
        /// execute enable
        const X = 1 << 3;   
        /// user accessability
        const U = 1 << 4;   
        /// Global mapping. We are going to use this for kernel code
        const G = 1 << 5;   
        /// Accessed
        const A = 1 << 6;   
        /// Dirty
        const D = 1 << 7;   
        /// Reserve 0, use for COW
        const R0 = 1 << 8;   
        /// Reserve 1
        const R1 = 1 << 9;   
    }
}

/// A pagetable entry for SV39 standard. Looked something like this:  
///` 63        5453                                                   1098          `  
///` | reserved ||                         PPN                         ||| DAGU XWRV`  
///`[0000 0000 00XX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX`  
#[derive(Copy, Clone)]
#[repr(C)]
pub struct PageTableEntry {
    pub bits: usize
}

impl Debug for PageTableEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PTE for {:?}, {:?}", self.ppn(), self.flags()))
    }
}

impl PageTableEntry {
	pub fn new(ppn: PhysPageNum, flags: PTEFlags) -> Self {
		Self {
			bits: (ppn.0 << 10) | flags.bits
		}
	}

	pub fn empty() -> Self {
		Self {bits: 0}
	}

	pub fn set_flags(&mut self, flags: PTEFlags) {
		let mask: usize = 0b11_1111_1111;
		self.bits = (self.bits & (!mask)) | flags.bits
	}

    pub fn set_ppn(&mut self, ppn: PhysPageNum) {
        let mask: usize = 0x003FFFFFFFFFC00;
        self.bits = (self.bits & (!mask)) | (ppn.0 << 10)
    }

	pub fn ppn(&self) -> PhysPageNum {
		((self.bits >> 10) & 0xFFF_FFFF_FFFF).into()
	}

	pub fn flags(&self) -> PTEFlags {
		PTEFlags::from_bits_truncate(self.bits)
	}

	pub fn valid(&self) -> bool {
		self.flags().contains(PTEFlags::V)
	}

	pub fn read(&self) -> bool {
		self.flags().contains(PTEFlags::R)
	}

	pub fn write(&self) -> bool {
		self.flags().contains(PTEFlags::W)
	}

	pub fn exec(&self) -> bool {
		self.flags().contains(PTEFlags::X)
	}

	pub fn user(&self) -> bool {
		self.flags().contains(PTEFlags::U)
	}

	pub fn global(&self) -> bool {
		self.flags().contains(PTEFlags::G)
	}

	pub fn access(&self) -> bool {
		self.flags().contains(PTEFlags::A)
	}

	pub fn dirty(&self) -> bool {
		self.flags().contains(PTEFlags::D)
	}

	pub fn r0(&self) -> bool {
		self.flags().contains(PTEFlags::R0)
	}

	pub fn r1(&self) -> bool {
		self.flags().contains(PTEFlags::R1)
	}
}

pub struct PageTable {
    root_ppn: PhysPageNum,
    pages: Vec<PageGuard>
}

impl PageTable {
    pub fn new() -> Self {
        let root = alloc_vm_page();
        Self {
            root_ppn: root.ppn,
            pages: vec![root]
        }
    }

    fn print_ptes(&self, page_addr: PhysPageNum, idx: [usize; 3], indentation: usize, log_level: LogLevel) {
        for i in 0..(PAGE_SIZE / size_of::<PageTableEntry>()) {
            let pte_addr = PhysAddr::from(page_addr) + i * size_of::<PageTableEntry>();
            let pte_content = unsafe{pte_addr.read_volatile::<PageTableEntry>()};
            if pte_content.valid() {
                if indentation < 3 {
                    log!(log_level, "{}|--- {:?} => non-leaf", "|   ".repeat(indentation-1), pte_content);
                    let mut new_idx = idx;
                    new_idx[indentation-1] = i;
                    self.print_ptes(pte_content.ppn(), new_idx, indentation + 1, log_level);
                } else {
                    log!(log_level, "{}|--- {:?} => vpn 0x{:x}", "|   ".repeat(indentation-1), pte_content, (idx[0] << 18) + (idx[1] << 9) + i);
                }
            }
        }
    }

    pub fn print(&self, log_level: LogLevel) {
        log!(log_level, "Pagetable @ {:?}", self.root_ppn);
        self.print_ptes(self.root_ppn, [0,0,0], 1, log_level);
    }

    pub fn satp(&self, pid: Option<ProcessID>) -> usize {
        if let Some(pid) = pid {
            (8usize << 60 )| (pid.0 << 44) | (self.root_ppn.0)
        } else {
            (8usize << 60 ) | (self.root_ppn.0)
        }
    }

    pub fn load(root_pageguard: PageGuard) -> Self {
        Self {
            root_ppn: root_pageguard.ppn.into(),
            pages: vec![root_pageguard]
        }
    }

    /// create PTE for the VPN if specified, and return the PhysAddr for the PTE
    #[deprecated]
    pub fn walk(&mut self, vpn: VirtPageNum, do_create: bool) -> Option<PhysAddr> {
        let indexes = vpn.indexes();
        let mut pt_ppn = self.root_ppn;
        for i in 0..3 {
            let pte_addr = PhysAddr::from(pt_ppn) + indexes[i] * size_of::<PageTableEntry>();
            if i == 2 {
                return Some(pte_addr);
            }
            let mut pte_content = unsafe{pte_addr.read_volatile::<PageTableEntry>()};
            if !pte_content.valid() {
                if do_create {
                    let pg = alloc_vm_page();
                    pte_content.bits = 0;
                    pte_content.set_ppn(pg.ppn);
                    pte_content.set_flags(PTEFlags::V);   // not leaf
                    unsafe{
                        pg.ppn.clear_content();
                        pte_addr.write_volatile(&pte_content);
                    }
                    self.pages.push(pg);
                } else {
                    return None;
                }
            }
            pt_ppn = pte_content.ppn();
        }
        unreachable!()
    }

    /// create PTE for the VPN if specified, and return the PhysAddr for the PTE
    pub fn walk_create(&mut self, vpn: VirtPageNum) -> PhysAddr {
        let indexes = vpn.indexes();
        let mut pt_ppn = self.root_ppn;
        for i in 0..3 {
            let pte_addr = PhysAddr::from(pt_ppn) + indexes[i] * size_of::<PageTableEntry>();
            if i == 2 {
                return pte_addr;
            }
            let mut pte_content = unsafe{pte_addr.read_volatile::<PageTableEntry>()};
            if !pte_content.valid() {
                let pg = alloc_vm_page();
                pte_content.bits = 0;
                pte_content.set_ppn(pg.ppn);
                pte_content.set_flags(PTEFlags::V);   // not leaf
                unsafe{
                    pg.ppn.clear_content();
                    pte_addr.write_volatile(&pte_content);
                }
                self.pages.push(pg);
            }
            pt_ppn = pte_content.ppn();
        }
        unreachable!()
    }

    /// create PTE for the VPN if specified, and return the PhysAddr for the PTE
    pub fn walk_find(&self, vpn: VirtPageNum) -> Option<PhysAddr> {
        let indexes = vpn.indexes();
        let mut pt_ppn = self.root_ppn;
        for i in 0..3 {
            let pte_addr = PhysAddr::from(pt_ppn) + indexes[i] * size_of::<PageTableEntry>();
            if i == 2 {
                return Some(pte_addr);
            }
            let pte_content = unsafe{pte_addr.read_volatile::<PageTableEntry>()};
            if !pte_content.valid() {
                return None;
            }
            pt_ppn = pte_content.ppn();
        }
        unreachable!()
    }

    pub fn map(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        // verbose!("Mapping {:?} -> {:?} with flag {:?}...", vpn, ppn, flags);
        let pte_addr = self.walk_create(vpn);
        let pte_content = PageTableEntry::new(ppn, flags | PTEFlags::V);
        unsafe{
            if pte_addr.read_volatile::<PageTableEntry>().valid() {
                panic!("remap!");
            }
            pte_addr.write_volatile(&pte_content);
        }
    }

    pub fn remap(&mut self, vpn: VirtPageNum, ppn: PhysPageNum, flags: PTEFlags) {
        // verbose!("Mapping {:?} -> {:?} with flag {:?}...", vpn, ppn, flags);
        let pte_addr = self.walk_create(vpn);
        let pte_content = PageTableEntry::new(ppn, flags | PTEFlags::V);
        unsafe{
            if !pte_addr.read_volatile::<PageTableEntry>().valid() {
                panic!("not remap!");
            }
            pte_addr.write_volatile(&pte_content);
        }
    }

    pub fn unmap(&mut self, vpn: VirtPageNum) {
        if let Some(pte_addr) = self.walk_find(vpn) {
            unsafe{pte_addr.write_volatile(&PageTableEntry::empty())}
        } else {
            panic!("unmapping free page")
        }
    }
}