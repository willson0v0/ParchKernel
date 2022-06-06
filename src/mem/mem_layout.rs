use core::{arch::asm};

use alloc::{vec::Vec, sync::Arc, string::String};
use riscv::register::{satp};
use crate::{config::{PHYS_END_ADDR, PAGE_SIZE, PROC_U_STACK_ADDR}, fs::RegularFile, mem::{TrampolineSegment, UTrampolineSegment, TrapContextSegment, IdenticalMappingSegment, segment::{SegmentFlags, ProgramSegment}, VirtAddr, types::VPNRange, VMASegment}, process::{get_processor, get_hart_id}, utils::{ErrorNum, RWLock}};
use super::{ArcSegment, MMAPType, PageTable, ProcKStackSegment, Segment, VirtPageNum, segment::ProcUStackSegment};
use crate::device::DEVICE_MANAGER;
use crate::utils::elf_rs_wrapper::read_elf;
use elf_rs::*;

use elf_rs::ElfFile;

pub struct MemLayout {
    pub pagetable: PageTable,
    pub segments: Vec<ArcSegment>
}


impl MemLayout {
    pub fn new() -> Self {
        verbose!("Initializing MemLayout...");
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
        // // text
        // verbose!("Registering Kernel text...");
        // layout.register_segment(
        //     IdenticalMappingSegment::new(
        //         VPNRange::new(
        //             VirtAddr::from(stext as usize).into(), 
        //             VirtAddr::from(etext as usize).to_vpn_ceil()
        //         ),
        //         SegmentFlags::R | SegmentFlags::X
        //     )
        // );
        // // rodata
        // verbose!("Registering Kernel rodata...");
        // layout.register_segment(
        //     IdenticalMappingSegment::new(
        //         VPNRange::new(
        //             VirtAddr::from(srodata as usize).into(), 
        //             VirtAddr::from(erodata as usize).to_vpn_ceil()
        //         ),
        //         SegmentFlags::R
        //     )
        // );
        // // data
        // verbose!("Registering Kernel data...");
        // layout.register_segment(
        //     IdenticalMappingSegment::new(
        //         VPNRange::new(
        //             VirtAddr::from(sdata as usize).into(), 
        //             VirtAddr::from(edata as usize).to_vpn_ceil()
        //         ),
        //         SegmentFlags::R
        //     )
        // );
        // // bss
        // verbose!("Registering Kernel bss...");
        // layout.register_segment(
        //     IdenticalMappingSegment::new(
        //         VPNRange::new(
        //             VirtAddr::from(sbss_with_stack as usize).into(), 
        //             VirtAddr::from(ebss as usize).to_vpn_ceil()
        //         ),
        //         SegmentFlags::R | SegmentFlags::W
        //     )
        // );
        // // Physical memories
        // verbose!("Registering Physical memory...");
        // layout.register_segment(
        //     IdenticalMappingSegment::new(
        //         VPNRange::new(
        //             VirtAddr::from(ekernel as usize).into(), 
        //             VirtAddr::from(PHYS_END_ADDR.0).to_vpn_ceil()
        //         ),
        //         SegmentFlags::R | SegmentFlags::W
        //     )
        // );
        // MMIOS (CLINT etc.)
        verbose!("Registering MMIO...");
        let dev_tree = DEVICE_MANAGER.acquire_r().get_dev_tree();
        let mmio_dev = dev_tree.contains_field("reg").unwrap();
        for dev in mmio_dev.iter() {
            let dev_l = dev.acquire_r();
            if !dev_l.unit_name.starts_with("flash") 
            && !dev_l.unit_name.starts_with("memory")
            && !dev_l.unit_name.starts_with("cpu") {
                let ranges = dev_l.reg_value().unwrap();
                for range in ranges {
                    verbose!("registering mmio 0x{:x} ~ 0x{:x}", range.address, range.address+range.size);
                    layout.register_segment(
                        IdenticalMappingSegment::new(
                            VPNRange::new(
                                VirtAddr::from(range.address).into(), 
                                VirtAddr::from(range.address+range.size).to_vpn_ceil()
                            ),
                            SegmentFlags::R | SegmentFlags::W
                        )
                    );
                }
            }
        }
        // for (start, end) in MMIO_RANGES {
        //     layout.register_segment(
        //         IdenticalMappingSegment::new(
        //             VPNRange::new(
        //                 VirtAddr::from(*start).into(), 
        //                 VirtAddr::from(*end).into()
        //             ),
        //             SegmentFlags::R | SegmentFlags::W
        //         )
        //     );
        // }

        // XXX: do_map here, or later?
        verbose!("Mapping all segment into pagetable...");
        layout.do_map();
        // debug!("Current vm mem usage: {:?}", stat_mem());
        layout
    }

    pub fn reset(&mut self) -> Result<(), ErrorNum> {
        // verbose!("Resetting memory layout...");
        // extern "C" {
        //     fn stext();
        //     fn srodata();
        //     fn sdata();
        //     fn sbss_with_stack();
        //     fn ekernel();
        // }
        // let mut basic = Vec::new();
        // basic.push(TRAMPOLINE_ADDR  );
        // basic.push(U_TRAMPOLINE_ADDR);
        // basic.push(TRAP_CONTEXT_ADDR);
        // basic.push(PROC_K_STACK_ADDR);
        // basic.push(PROC_U_STACK_ADDR);
        // basic.push(VirtAddr::from(stext as usize).into());
        // basic.push(VirtAddr::from(srodata as usize).into());
        // basic.push(VirtAddr::from(sdata as usize).into());
        // basic.push(VirtAddr::from(sbss_with_stack as usize).into());
        // basic.push(VirtAddr::from(ekernel as usize).into());

        // for (start, _) in MMIO_RANGES {
        //     basic.push(VirtAddr::from(*start).into());
        // }

        // let basic = basic.into_iter().map(|va| VirtPageNum::from(va)).collect::<Vec<VirtPageNum>>();

        let mut to_clear = Vec::new();
        for seg in self.segments.iter() {
            verbose!("reset checking {:?}...", seg);
            if seg.clone().as_program().is_ok() {
                to_clear.push(seg.clone());
            }
            if seg.clone().as_vma().is_ok() {
                to_clear.push(seg.clone());
            }
            if seg.clone().as_managed().is_ok() {
                to_clear.push(seg.clone());
            }
        }
        for seg in to_clear {
            self.remove_segment(seg)?;
        }
        Ok(())
    }

    pub fn register_segment(&mut self, seg: ArcSegment) {
        self.segments.push(seg);
    }

    pub fn map_proc_stack(&mut self) {
        self.register_segment(ProcKStackSegment::new());
        self.register_segment(ProcUStackSegment::new());
        self.do_map();
    }

    pub fn do_map(&mut self) {
        debug!("Memlayout @ {:?} mapping.", self.pagetable.root_ppn);
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
            info!("Kernel virtual memory layout has been activated on core {}.", get_hart_id());
        }   
    }

    pub fn occupied(&self, vpn: VirtPageNum) -> bool {
        self.pagetable.translate(vpn).is_ok()
    }

    // length in byte
    pub fn get_space(&self, length: usize) -> Result<VirtPageNum, ErrorNum> {
        let vpn_top = VirtPageNum::from(VirtAddr::from(PROC_U_STACK_ADDR - PAGE_SIZE));
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

    pub fn get_segment(&self, vpn: VirtPageNum) -> Result<ArcSegment, ErrorNum> {
        for seg in self.segments.iter() {
            if seg.contains(vpn) {
                return Ok(seg.clone());
            }
        }
        return Err(ErrorNum::ENOSEG);
    }

    pub fn unmap_segment_by_vpn(&mut self, vpn: VirtPageNum) -> Result<(), ErrorNum> {
        let seg = self.get_segment(vpn)?;
        seg.do_unmap(&mut self.pagetable)?;
        Ok(())
    }

    pub fn unmap_segment(&mut self, seg: &ArcSegment) -> Result<(), ErrorNum> {
        seg.do_unmap(&mut self.pagetable)?;
        Ok(())
    }

    pub fn remove_segment_by_vpn(&mut self, vpn: VirtPageNum) -> Result<(), ErrorNum> {
        let seg = self.get_segment(vpn)?;
        self.remove_segment(seg)
    }

    pub fn remove_segment(&mut self, seg: ArcSegment) -> Result<(), ErrorNum> {
        if self.segments.contains(&seg) {
            self.unmap_segment(&seg)?;
            self.segments.retain(|x| x.clone() != seg);
            Ok(())
        } else {
            Err(ErrorNum::ENOSEG)
        }
    }

    pub fn mmap_file(&mut self, file: Arc<dyn RegularFile>, offset: usize, length: usize, mmap_type: MMAPType) -> Result<VirtPageNum, ErrorNum> {
        if mmap_type == MMAPType::Shared && offset % PAGE_SIZE == 0 {
            return Err(ErrorNum::ENOTALIGNED);
        }
        let stat = file.stat()?;
        let start_vpn = self.get_space(stat.file_size)?;
        self.register_segment(VMASegment::new_at(
            start_vpn,
            file.clone(),
            stat.open_mode.into(),
            offset,
            length,
            mmap_type
        )?);
        Ok(start_vpn)
    }

    pub fn map_elf(&mut self, elf_file: Arc<dyn RegularFile>) -> Result<(VirtAddr, VirtAddr), ErrorNum> {
        verbose!("Mapping elf into memory space");
        // first map it for easy reading...
        let stat = elf_file.stat()?;
        let first_map = if get_processor().current().is_none() {
            get_processor().map_file(elf_file.clone())
        } else {
            // a little bit faster without copying.
            let res = self.mmap_file(elf_file.clone(), 0, stat.file_size, MMAPType::Private)?;
            self.do_map();
            res
        };
        verbose!("init map start {:?} len {:?}", first_map, stat.file_size);

        // some dirty trick for zero copy
        let start_va: VirtAddr = first_map.into();
        let start_ptr = start_va.0 as *mut u8;
        let buffer = unsafe{core::slice::from_raw_parts(start_ptr, stat.file_size)};

        let elf = read_elf(buffer)?;

        debug!("Loading {:?} into mem_layout...", elf_file);
        
        verbose!("elf Info: {:?}", elf);
        verbose!("Header Info: {:?}", elf.elf_header());

        let mut data_end: VirtAddr = 0.into();
        for h in elf.section_header_iter() {
            let mapping = String::from_utf8(h.section_name().to_vec()).map_err(|_| ErrorNum::ENOEXEC)?;
            if mapping.contains("data") {
                data_end = ((h.addr() + h.size()) as usize).into();
            }
        }

        for p in elf.program_header_iter() {
            verbose!("Handling PH {:x?}", p);
            if p.ph_type() == ProgramType::LOAD {
                let seg_start: VirtAddr = (p.vaddr() as usize).into();
                if seg_start.0 % PAGE_SIZE != 0 {
                    panic!("Program header not aligned!")
                }
                let seg_start: VirtPageNum = seg_start.into();
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

                let segment = ProgramSegment::new_at(
                    seg_start, 
                    elf_file.clone(), 
                    seg_flag, 
                    p.offset() as usize, 
                    p.filesz() as usize,
                    p.memsz() as usize
                ).unwrap();
                self.register_segment(segment);
            }
        }
        let entry_point = elf.entry_point() as usize;
        // free the first mmap...
        if get_processor().current().is_none() {
            get_processor().unmap_file(first_map);
        } else {
            self.remove_segment_by_vpn(first_map).unwrap();
        }
        Ok((entry_point.into(), data_end.into()))
    }

    pub fn fork(&mut self) -> Result<Self, ErrorNum> {
        debug!("Forking memlayout @ {:?}", self.pagetable.root_ppn);
        let mut layout = Self {
            pagetable: PageTable::new(),
            segments: Vec::new()
        };
        debug!("New memlayout @ {:?}", layout.pagetable.root_ppn);

        for seg in self.segments.iter() {
            layout.register_segment(seg.clone_seg(&mut self.pagetable)?);
        }
        layout.do_map();
        unsafe { asm!("sfence.vma"); }
        Ok(layout)
    }

    pub fn do_lazy(&mut self, vpn: VirtPageNum) -> Result<(), ErrorNum> {
        for seg in self.segments.iter() {
            if seg.contains(vpn) {
                return seg.do_lazy(vpn, &mut self.pagetable);
            }
        }
        error!("Cannot find lazy entry for {:?}", vpn);
        Err(ErrorNum::ENOSEG)
    }

    pub fn unmap_vma(&mut self, head: VirtAddr, length: usize) -> Result<(), ErrorNum> {
        let seg = self.get_segment(head.into())?.as_vma()?;
        seg.unmap_part(head, length, &mut self.pagetable)?;
        if seg.is_empty() {
            self.remove_segment(ArcSegment(seg.as_segment()))?;
        }
        Ok(())
    }
}