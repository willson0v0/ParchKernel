use core::{arch::asm};

use alloc::{vec::Vec, sync::Arc};
use riscv::register::{satp};
use crate::{utils::{SpinMutex, ErrorNum}, config::{PHYS_END_ADDR, MMIO_RANGES, TRAP_CONTEXT_ADDR, PAGE_SIZE}, mem::{TrampolineSegment, UTrampolineSegment, TrapContextSegment, IdenticalMappingSegment, segment::SegmentFlags, VirtAddr, types::VPNRange}};
use lazy_static::*;
use super::{PageTable, Segment, VirtPageNum};


lazy_static! {
    /// The scheduler kernel space memory layout.
    pub static ref SCHEDULER_MEM_LAYOUT: Arc<SpinMutex<MemLayout>> = Arc::new(SpinMutex::new("Scheduler memlayout", MemLayout::new()));
}


pub struct MemLayout {
    pub pagetable: PageTable,
    pub segments: Vec<Arc<dyn Segment>>
}

impl MemLayout {
    pub fn new() -> Self {
        let mut layout = Self {
            pagetable: PageTable::new(),
            segments: Vec::new()
        };

        extern "C" {
            fn stext();
            fn etext();
            fn srodata();
            fn erodata();
            fn sdata();
            fn edata();
            fn sbss_with_stack();
            fn ebss();
            fn ekernel();
        }
        // trampoline
        verbose!("Registering Trampoline...");
        layout.add_segment(TrampolineSegment::new());
        // u_trampoline
        verbose!("Registering UTrampoline...");
        layout.add_segment(UTrampolineSegment::new());
        // trap_context
        verbose!("Registering TrapContext...");
        layout.add_segment(TrapContextSegment::new());
        // text
        verbose!("Registering Kernel text...");
        layout.add_segment(
            IdenticalMappingSegment::new(
                VPNRange::new(
                    VirtAddr::from(stext as usize).into(), 
                    VirtAddr::from(etext as usize).to_vpn_ceil()
                ),
                SegmentFlags::R | SegmentFlags::X
            )
        );
        // rodata
        verbose!("Registering Kernel rodata...");
        layout.add_segment(
            IdenticalMappingSegment::new(
                VPNRange::new(
                    VirtAddr::from(srodata as usize).into(), 
                    VirtAddr::from(erodata as usize).to_vpn_ceil()
                ),
                SegmentFlags::R
            )
        );
        // data
        verbose!("Registering Kernel data...");
        layout.add_segment(
            IdenticalMappingSegment::new(
                VPNRange::new(
                    VirtAddr::from(sdata as usize).into(), 
                    VirtAddr::from(edata as usize).to_vpn_ceil()
                ),
                SegmentFlags::R
            )
        );
        // bss
        verbose!("Registering Kernel bss...");
        layout.add_segment(
            IdenticalMappingSegment::new(
                VPNRange::new(
                    VirtAddr::from(sbss_with_stack as usize).into(), 
                    VirtAddr::from(ebss as usize).to_vpn_ceil()
                ),
                SegmentFlags::R | SegmentFlags::W
            )
        );
        // Physical memories
        verbose!("Registering Physical memory...");
        layout.add_segment(
            IdenticalMappingSegment::new(
                VPNRange::new(
                    VirtAddr::from(ekernel as usize).into(), 
                    VirtAddr::from(PHYS_END_ADDR.0).to_vpn_ceil()
                ),
                SegmentFlags::R | SegmentFlags::W
            )
        );
        // MMIOS (CLINT etc.)
        verbose!("Registering MMIO...");
        for (start, end) in MMIO_RANGES {
            layout.add_segment(
                IdenticalMappingSegment::new(
                    VPNRange::new(
                        VirtAddr::from(*start).into(), 
                        VirtAddr::from(*end).to_vpn_ceil()
                    ),
                    SegmentFlags::R | SegmentFlags::W
                )
            );
        }

        // XXX: do_map here, or later?
        verbose!("Mapping all segment into pagetable...");
        layout.do_map();

        layout
    }

    pub fn add_segment(&mut self, seg: Arc<dyn Segment + Send>) {
        self.segments.push(seg);
    }

    pub fn do_map(&mut self) {
        for seg in self.segments.iter() {
            verbose!("Now mapping {:?} ...", seg);
            seg.do_map(&mut self.pagetable);
            debug!("Done mapping {:?}.", seg);
        }
    }

    pub fn activate(&self) {
        debug!("Activating pagetable @ 0x{:x}", self.pagetable.satp());
        let satp = self.pagetable.satp();
        unsafe {
            satp::write(satp);
            asm!("sfence.vma");
        }
        if satp::read().mode() != satp::Mode::Sv39 {
            fatal!("Failed switch to SV39!");
        } else {
            info!("Kernel virtual memory layout has been activated.");
        }
    }

    fn occupied(&self, vpn: VirtPageNum) -> bool {
        self.pagetable.walk_find(vpn).is_some()
    }

    // length in byte
    pub fn get_space(&self, length: usize) -> Result<VirtPageNum, ErrorNum> {
        let vpn_top = VirtPageNum::from(TRAP_CONTEXT_ADDR - PAGE_SIZE);
        let vpn_bottom = VirtPageNum::from(VirtAddr::from(PHYS_END_ADDR.0));
        let page_count = (length / PAGE_SIZE) + 2; // guard page
        for vpn_s in VPNRange::new(vpn_top - page_count, vpn_bottom) {
            let mut good = true;
            for vpn in VPNRange::new(vpn_s, vpn_s + page_count) {
                if self.occupied(vpn) {
                    good = false;
                    break;
                }
            }
            if good {
                return Ok(vpn_s + 1);
            } else {
                continue;
            }
        }
        Err(ErrorNum::ENOMEM)
    }

    pub fn get_segment(&self, start_vpn: VirtPageNum) -> Option<Arc<dyn Segment>> {
        for seg in self.segments.iter() {
            if seg.start_vpn() == start_vpn {
                return Some(seg.clone());
            }
        }
        return None;
    }
}