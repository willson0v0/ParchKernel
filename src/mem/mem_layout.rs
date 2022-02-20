use core::arch::asm;

use alloc::{vec::Vec, sync::Arc, boxed::Box};
use riscv::register::{satp, satp::Mode};
use crate::{utils::{SpinMutex, Mutex, LogLevel}, config::{TRAMPOLINE_ADDR, PHYS_END_ADDR, MMIO_RANGES}, mem::{PhysAddr, TrampolineSegment, UTrampolineSegment, TrapContextSegment, IdenticalMappingSegment, segment::SegmentFlags, VirtAddr, types::VPNRange}};
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

    pub fn add_segment(&mut self, seg: Box<dyn Segment + Send>) {
        self.segments.push(Arc::new(SpinMutex::new("memlayout segment", seg)));
    }

    pub fn do_map(&mut self) {
        for seg in self.segments.iter() {
            let mut seg_locked = seg.acquire();
            verbose!("Now mapping {:?} ...", seg_locked.as_ref());
            seg_locked.do_map(&mut self.pagetable);
            debug!("Done mapping {:?}.", seg_locked.as_ref());
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
}