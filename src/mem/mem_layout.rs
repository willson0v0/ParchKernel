use core::{arch::asm};

use alloc::{vec::Vec, sync::Arc};
use riscv::register::{satp};
use crate::{utils::{SpinMutex, ErrorNum, Mutex}, config::{PHYS_END_ADDR, MMIO_RANGES, PAGE_SIZE, PROC_K_STACK_ADDR, TRAMPOLINE_ADDR, U_TRAMPOLINE_ADDR, TRAP_CONTEXT_ADDR, PROC_U_STACK_ADDR}, mem::{TrampolineSegment, UTrampolineSegment, TrapContextSegment, IdenticalMappingSegment, segment::SegmentFlags, VirtAddr, types::VPNRange, ManagedSegment, stat_mem}, fs::RegularFile, process::{get_processor}};
use lazy_static::*;
use super::{PageTable, Segment, VirtPageNum, ProcKStackSegment, segment::ProcUStackSegment};

use crate::utils::elf_rs_wrapper::read_elf;
use elf_rs::*;

use elf_rs::ElfFile;


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
        layout.register_segment(TrampolineSegment::new());
        // u_trampoline
        verbose!("Registering UTrampoline...");
        layout.register_segment(UTrampolineSegment::new());
        // trap_context
        verbose!("Registering TrapContext...");
        layout.register_segment(TrapContextSegment::new());
        // text
        verbose!("Registering Kernel text...");
        layout.register_segment(
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
        layout.register_segment(
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
        layout.register_segment(
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
        layout.register_segment(
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
        layout.register_segment(
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
            layout.register_segment(
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
        debug!("Current vm mem usage: {:?}", stat_mem());
        layout
    }

    pub fn reset(&mut self) -> Result<(), ErrorNum> {
        extern "C" {
            fn stext();
            fn srodata();
            fn sdata();
            fn sbss_with_stack();
            fn ekernel();
        }
        let mut basic = Vec::new();
        basic.push(TRAMPOLINE_ADDR  );
        basic.push(U_TRAMPOLINE_ADDR);
        basic.push(TRAP_CONTEXT_ADDR);
        basic.push(PROC_K_STACK_ADDR);
        basic.push(PROC_U_STACK_ADDR);
        basic.push(VirtAddr::from(stext as usize).into());
        basic.push(VirtAddr::from(srodata as usize).into());
        basic.push(VirtAddr::from(sdata as usize).into());
        basic.push(VirtAddr::from(sbss_with_stack as usize).into());
        basic.push(VirtAddr::from(ekernel as usize).into());

        for (start, _) in MMIO_RANGES {
            basic.push(VirtAddr::from(*start).into());
        }

        let basic = basic.into_iter().map(|va| VirtPageNum::from(va)).collect::<Vec<VirtPageNum>>();

        let mut to_clear = Vec::new();
        for seg in self.segments.iter() {
            let start_vpn = seg.start_vpn();
            if !basic.contains(&start_vpn) {
                to_clear.push(start_vpn);
            }
        }
        for vpn in to_clear {
            self.remove_segment(vpn)?;
        }
        Ok(())
    }

    pub fn register_segment(&mut self, seg: Arc<dyn Segment>) {
        self.segments.push(seg);
    }

    pub fn map_proc_stack(&mut self) {
        self.register_segment(ProcKStackSegment::new());
        self.register_segment(ProcUStackSegment::new());
        self.do_map();
    }

    pub fn do_map(&mut self) {
        for seg in self.segments.iter() {
            let map_res = seg.do_map(&mut self.pagetable);
            if map_res.is_ok() {
                verbose!("Done mapping {:?}.", seg);
            } else if map_res.err().unwrap() != ErrorNum::EMMAPED {
                panic!("Unknown mapping error {:?}", map_res.err().unwrap());
            }
        }
    }

    pub fn activate(&self) {
        info!("This Pagetable uses {} page", self.pagetable.pages.len());
        let satp = self.pagetable.satp(get_processor().current().and_then(|pcb| Some(pcb.pid)));
        debug!("Activating pagetable @ 0x{:x}", satp);
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
        let vpn_top = VirtPageNum::from(PROC_K_STACK_ADDR - PAGE_SIZE);
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

    pub fn get_segment(&self, start_vpn: VirtPageNum) -> Result<Arc<dyn Segment>, ErrorNum> {
        for seg in self.segments.iter() {
            if seg.start_vpn() == start_vpn {
                return Ok(seg.clone());
            }
        }
        return Err(ErrorNum::ENOSEG);
    }

    pub fn unmap_segment(&mut self, start_vpn: VirtPageNum) -> Result<(), ErrorNum> {
        let seg = self.get_segment(start_vpn)?;
        seg.do_unmap(&mut self.pagetable)?;
        Ok(())
    }

    pub fn remove_segment(&mut self, start_vpn: VirtPageNum) -> Result<(), ErrorNum> {
        self.unmap_segment(start_vpn)?;
        self.segments.retain(|x| x.start_vpn() != start_vpn);
        Ok(())
    }

    pub fn map_elf(&mut self, elf_file: Arc<dyn RegularFile>) -> Result<VirtAddr, ErrorNum> {
        verbose!("Mapping elf into memory space");
        // first map it for easy reading...
        let first_map = elf_file.clone().register_mmap(self)?;
        self.do_map();
        let stat = elf_file.stat()?;

        // some dirty trick for zero copy
        let start_va: VirtAddr = first_map.into();
        let start_ptr = start_va.0 as *mut u8;
        let buffer = unsafe{core::slice::from_raw_parts(start_ptr, stat.file_size)};

        let elf = read_elf(buffer)?;

        debug!("Loading {:?} into mem_layout...", elf_file);
        
        verbose!("elf Info: {:?}", elf);
        verbose!("Header Info: {:?}", elf.elf_header());
        for p in elf.program_header_iter() {
            verbose!("Handling PH {:x?}", p);
            if p.ph_type() == ProgramType::LOAD {
                let seg_start: VirtAddr = (p.vaddr() as usize).into();
                let seg_end: VirtAddr = seg_start + p.memsz() as usize;
                if seg_start.0 % PAGE_SIZE != 0 {
                    panic!("Program header not aligned!")
                }
                let seg_start: VirtPageNum = seg_start.into();
                let seg_end = seg_end.to_vpn_ceil();
                let mut seg_flag = SegmentFlags::U;
                if p.flags().contains(ProgramHeaderFlags::EXECUTE) {
                    seg_flag = seg_flag | SegmentFlags::X;
                }
                if p.flags().contains(ProgramHeaderFlags::READ) {
                    seg_flag = seg_flag | SegmentFlags::R;
                }
                if p.flags().contains(ProgramHeaderFlags::WRITE) {
                    seg_flag = seg_flag | SegmentFlags::W;
                }
                let segment = ManagedSegment::new(VPNRange::new(seg_start, seg_end + 1), SegmentFlags::W | SegmentFlags::R);
                self.register_segment(segment.clone());
                self.do_map();
                // copy data into it
                let src = unsafe{ buffer.as_ptr().add(p.offset() as usize)};
                let dst = VirtAddr::from(seg_start).0 as *mut u8;
                let len = p.filesz() as usize;
                unsafe{core::ptr::copy_nonoverlapping(src, dst, len)}
                segment.alter_permission(seg_flag, &mut self.pagetable);
            }
        }
        let entry_point = elf.entry_point() as usize;
        // free the first mmap...
        self.remove_segment(first_map)?;
        Ok(entry_point.into())
    }

    pub fn fork(&self) -> Result<Self, ErrorNum> {
        let mut layout = Self {
            pagetable: PageTable::new(),
            segments: Vec::new()
        };

        for seg in self.segments.iter() {
            layout.register_segment(seg.clone_seg()?);
        }
        layout.do_map();
        Ok(layout)
    }
}