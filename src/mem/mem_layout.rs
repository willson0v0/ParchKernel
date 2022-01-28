use alloc::{vec::Vec, sync::Arc, boxed::Box};
use riscv::register::satp;
use crate::{utils::{SpinMutex, Mutex}, config::{TRAMPOLINE_ADDR, PHYS_END_ADDR, MMIO_RANGES}, mem::{PhysAddr, TrampolineSegment, UTrampolineSegment, TrapContextSegment, IdenticalMappingSegment, segment::SegmentFlags, VirtAddr, types::VPNRange}};
use lazy_static::*;
use super::{PageTable, Segment, PTEFlags};


lazy_static! {
    /// The scheduler kernel space memory layout.
    pub static ref SCHEDULER_MEM_LAYOUT: Arc<SpinMutex<MemLayout>> = Arc::new(SpinMutex::new("Scheduler memlayout", MemLayout::new()));
}


pub struct MemLayout {
    pagetable: PageTable,
    segments: Vec<Arc<SpinMutex<Box<dyn Segment + Send>>>>
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
        layout.add_segment(TrampolineSegment::new());
        // u_trampoline
        layout.add_segment(UTrampolineSegment::new());
        // trap_context
        layout.add_segment(TrapContextSegment::new());
        // text
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
        layout.do_map();

        layout
    }

    pub fn add_segment(&mut self, seg: Box<dyn Segment + Send>) {
        self.segments.push(Arc::new(SpinMutex::new("memlayout segment", seg)));
    }

    pub fn do_map(&mut self) {
        for seg in self.segments.iter() {
            seg.acquire().do_map(&mut self.pagetable);
        }
    }

    pub fn activate(&self) {
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
}