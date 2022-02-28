use core::fmt::{self, Debug, Formatter};

use alloc::{sync::{Arc}, collections::BTreeMap, vec::Vec, borrow::ToOwned};
use bitflags::*;
use crate::{config::{PAGE_SIZE, PROC_K_STACK_SIZE, PROC_K_STACK_ADDR, PROC_U_STACK_SIZE, PROC_U_STACK_ADDR}, utils::{SpinMutex, Mutex}};
use crate::{fs::{RegularFile}, utils::ErrorNum, config::{TRAMPOLINE_ADDR, U_TRAMPOLINE_ADDR, TRAP_CONTEXT_ADDR}};

use super::{types::{VPNRange, VirtPageNum, PhysPageNum}, PageGuard, pagetable::{PageTable, PTEFlags}, alloc_vm_page, PhysAddr};


bitflags! {
    /// Segment flags indicaing privilege.
    pub struct SegmentFlags: usize {
        /// Can this segment be read?
        const R = 1 << 1;
        /// Can this segment be written?
        const W = 1 << 2;
        /// Can this segment be executed?
        const X = 1 << 3;
        /// Can this segment be accessed from user mode?
        const U = 1 << 4;
    }
}

impl Into<PTEFlags> for SegmentFlags {
    fn into(self) -> PTEFlags {
        PTEFlags::from_bits(self.bits).unwrap()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentStatus {
    Initialized,
    Mapped,
    Zombie
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentType {
    Identical,
    Managed,
    VMA,
    Trampoline,
    UTrampoline,
    TrapContext
}


pub trait Segment: Debug + Send + Sync {
    fn as_segment   <'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a;
    fn as_identical <'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment >, ErrorNum> where Self: 'a;
    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment          >, ErrorNum> where Self: 'a;
    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment              >, ErrorNum> where Self: 'a;
    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum>;
    fn do_unmap(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum>;
    fn status(&self) -> SegmentStatus;
    fn seg_type(&self) -> SegmentType;
    fn start_vpn(&self) -> VirtPageNum;
    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum>;
}

pub struct IdenticalMappingSegment (SpinMutex<IdenticalMappingSegmentInner>);

struct IdenticalMappingSegmentInner {
    range: VPNRange,
    flag: SegmentFlags,
    status: SegmentStatus
}

pub struct ManagedSegment (pub SpinMutex<ManagedSegmentInner>);
pub struct ManagedSegmentInner {
    range: VPNRange,
    frames: BTreeMap<VirtPageNum, PageGuard>,
    flag: SegmentFlags,
    status: SegmentStatus,
    pub clone_source: Option<Arc<ManagedSegment>>
}

pub struct VMASegment (SpinMutex<VMASegmentInner>);
pub struct VMASegmentInner {
    frames: BTreeMap<VirtPageNum, PageGuard>,
    file: Arc<dyn RegularFile>,
    flag: SegmentFlags,
    status: SegmentStatus,
    start_vpn: VirtPageNum
}

pub struct TrampolineSegment (SpinMutex<TrampolineSegmentInner>);
pub struct TrampolineSegmentInner {
    status: SegmentStatus
}

pub struct UTrampolineSegment (SpinMutex<UTrampolineSegmentInner>);
pub struct UTrampolineSegmentInner {
    status: SegmentStatus
}

pub struct TrapContextSegment (pub SpinMutex<TrapContextSegmentInner>);
pub struct TrapContextSegmentInner {
    pub status: SegmentStatus,
    pub page: Option<PageGuard>,
    pub clone_source: Option<Arc<TrapContextSegment>>
}

pub struct ProcKStackSegment (SpinMutex<ProcKStackSegmentInner>);
pub struct ProcKStackSegmentInner {
    pub status: SegmentStatus,
    pub pages: Vec<PageGuard>
}

pub struct ProcUStackSegment (pub SpinMutex<ProcUStackSegmentInner>);
pub struct ProcUStackSegmentInner {
    pub status: SegmentStatus,
    pub pages: Vec<PageGuard>,
    pub clone_source: Option<Arc<ProcUStackSegment>>
}

impl Debug for IdenticalMappingSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} Identical segment {:?} ~ {:?} with flag {:?}", inner.status, inner.range.start(), inner.range.end(), inner.flag))
    }
}

impl Debug for ManagedSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} Managed segment {:?} ~ {:?} with flag {:?}", inner.status, inner.range.start(), inner.range.end(), inner.flag))
    }
}

impl Debug for VMASegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // TODO: Add file desc
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} VMA segment @ {:?} with flag {:?}", inner.status, inner.start_vpn, inner.flag))
    }
}

impl Debug for TrampolineSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} Trampoline segment", inner.status))
    }
}

impl Debug for UTrampolineSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} UTrampoline segment", inner.status))
    }
}

impl Debug for TrapContextSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} TrapContext segment", inner.status))
    }
}

impl Debug for ProcKStackSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} ProcKStack segment", inner.status))
    }
}

impl Debug for ProcUStackSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} ProcUStack segment", inner.status))
    }
}

impl Segment for IdenticalMappingSegment {
    fn as_segment<'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical<'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment>, ErrorNum>
    where Self: 'a {
        Ok(self)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }
        for vpn in inner.range {
            let ppn = PhysPageNum(vpn.0);
            pagetable.map(vpn, ppn, inner.flag.into())
        }
        inner.status = SegmentStatus::Mapped;
        Ok(())
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        panic!("Parch don't unmap identitical mapping segment");
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::Identical
    }

    fn start_vpn(&self) -> VirtPageNum {
        self.0.acquire().range.start()
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        let inner = self.0.acquire();
        Ok(Self::new(inner.range, inner.flag))
    }
}

impl Segment for ManagedSegment {
    fn as_segment<'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical<'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment>, ErrorNum>
    where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment>, ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }
        for vpn in inner.range {
            let pageguard = alloc_vm_page();
            let ppn = pageguard.ppn;

            if let Some(source) = inner.clone_source.take() {
                let source_ppn = source.0.acquire().frames.get(&vpn).unwrap().ppn;
                unsafe {PhysPageNum::copy_page(&source_ppn, &ppn)}
            }

            pagetable.map(vpn, ppn, inner.flag.into());
            inner.frames.insert(vpn, pageguard);
        }
        inner.status = SegmentStatus::Mapped;
        Ok(())
    }

    fn do_unmap(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        assert!(inner.status == SegmentStatus::Mapped);
        for vpn in inner.range {
            inner.frames.remove(&vpn).unwrap();
            pagetable.unmap(vpn);
        }
        inner.status = SegmentStatus::Zombie;
        Ok(())
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::Managed
    }

    fn start_vpn(&self) -> VirtPageNum {
        self.0.acquire().range.start()
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        let inner = self.0.acquire();
        Ok(Self::new(inner.range, inner.flag, Some(self.clone())))
    }
}

impl Segment for VMASegment {
    fn as_segment<'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical<'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment>, ErrorNum>
    where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment>, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment>, ErrorNum> where Self: 'a {
        Ok(self)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }
        
        let mut vpn = inner.start_vpn;
        let mut offset = 0;
        while offset < inner.file.stat()?.file_size {
            let pg = inner.file.get_page(offset)?;
            inner.frames.insert(vpn, pg.clone());
            pagetable.map(vpn, pg.ppn, inner.flag.into());
            offset += PAGE_SIZE;
            vpn += 1;
        }

        inner.status = SegmentStatus::Mapped;
        Ok(())
    }

    fn do_unmap(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Mapped {
            return Err(ErrorNum::ENOSEG);
        }
        assert!(inner.status == SegmentStatus::Mapped);
        for (vpn, _pg) in &inner.frames {
            pagetable.unmap(*vpn);
        }
        inner.frames.clear();
        inner.status = SegmentStatus::Zombie;
        Ok(())
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::VMA
    }

    fn start_vpn(&self) -> VirtPageNum {
        self.0.acquire().start_vpn
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        let inner = self.0.acquire();
        Ok(Self::new_at(inner.start_vpn, inner.file.clone(), inner.flag))
    }
}

impl Segment for TrampolineSegment {
    fn as_segment   <'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical <'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment          >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment              >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        extern "C" {
            fn strampoline();
        }
        
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }
        pagetable.map(
            TRAMPOLINE_ADDR.into(),
            PhysAddr::from(strampoline as usize).into(), 
            PTEFlags::R | PTEFlags::X
        );
        inner.status = SegmentStatus::Mapped;
        Ok(())
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        panic!("Don't unmap trampoline!")
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::Trampoline
    }

    fn start_vpn(&self) -> VirtPageNum {
        U_TRAMPOLINE_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        Ok(Self::new())
    }
}

impl Segment for UTrampolineSegment {
    fn as_segment   <'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical <'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment          >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment              >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        extern "C" {
            fn sutrampoline();
        }
        
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }
        pagetable.map(
            U_TRAMPOLINE_ADDR.into(),
            PhysAddr::from(sutrampoline as usize).into(), 
            PTEFlags::R | PTEFlags::X | PTEFlags::U
        );
        inner.status = SegmentStatus::Mapped;
        Ok(())
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        panic!("Don't unmap u_trampoline!")
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::UTrampoline
    }

    fn start_vpn(&self) -> VirtPageNum {
        U_TRAMPOLINE_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        Ok(Self::new())
    }
}


impl Segment for TrapContextSegment {
    fn as_segment   <'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical <'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment          >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment              >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }
        let pageguard = alloc_vm_page();
        let ppn = pageguard.ppn;
        if let Some(source) = inner.clone_source.take() {
            let source_ppn = source.0.acquire().page.as_ref().unwrap().ppn;
            unsafe {PhysPageNum::copy_page(&source_ppn, &ppn)}
        }
        pagetable.map(
            TRAP_CONTEXT_ADDR.into(),
            ppn, 
            PTEFlags::R | PTEFlags::W
        );
        inner.status = SegmentStatus::Mapped;
        inner.page = Some(pageguard);
        Ok(())
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        panic!("Don't unmap trap_context!")
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::TrapContext
    }

    fn start_vpn(&self) -> VirtPageNum {
        TRAP_CONTEXT_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        Ok(Self::new(Some(self.clone())))
    }
}

impl Segment for ProcKStackSegment {
    fn as_segment   <'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical <'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment          >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment              >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }

        assert!(PROC_K_STACK_SIZE % PAGE_SIZE == 0, "Proc KStack size misaligned");
        let page_count = PROC_K_STACK_SIZE / PAGE_SIZE;
        let start_vpn: VirtPageNum = PROC_K_STACK_ADDR.into();
        for i in 0..page_count {
            let pageguard = alloc_vm_page();
            let ppn = pageguard.ppn;
            let vpn = start_vpn + i;
            pagetable.map(
                vpn,
                ppn, 
                PTEFlags::R | PTEFlags::W
            );
            inner.pages.push(pageguard)
        }
        inner.status = SegmentStatus::Mapped;
        Ok(())
    }

    fn do_unmap(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        let page_count = PROC_K_STACK_SIZE / PAGE_SIZE;
        let start_vpn: VirtPageNum = PROC_K_STACK_ADDR.into();
        for i in 0..page_count {
            pagetable.unmap(start_vpn + i);
        }
        inner.pages.clear();
        inner.status = SegmentStatus::Zombie;
        Ok(())
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::TrapContext
    }

    fn start_vpn(&self) -> VirtPageNum {
        TRAP_CONTEXT_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        Ok(Self::new())
    }
}

impl Segment for ProcUStackSegment {
    fn as_segment   <'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a {
        self
    }

    fn as_identical <'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment          >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment              >, ErrorNum> where Self: 'a {
        Err(ErrorNum::EWRONGSEG)
    }

    fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return Err(ErrorNum::EMMAPED);
        }

        assert!(PROC_U_STACK_SIZE % PAGE_SIZE == 0, "Proc UStack size misaligned");
        let page_count = PROC_U_STACK_SIZE / PAGE_SIZE;
        let start_vpn: VirtPageNum = PROC_U_STACK_ADDR.into();
        for i in 0..page_count {
            let pageguard = alloc_vm_page();
            let ppn = pageguard.ppn;

            if let Some(source) = inner.clone_source.take() {
                let source_ppn = source.0.acquire().pages.get(i).unwrap().ppn;
                unsafe {PhysPageNum::copy_page(&source_ppn, &ppn)};
            }

            pagetable.map(
                start_vpn + i,
                ppn, 
                PTEFlags::R | PTEFlags::W | PTEFlags::U
            );
            inner.pages.push(pageguard)
        }
        inner.status = SegmentStatus::Mapped;
        Ok(())
    }

    fn do_unmap(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        let page_count = PROC_U_STACK_SIZE / PAGE_SIZE;
        let start_vpn: VirtPageNum = PROC_U_STACK_ADDR.into();
        for i in 0..page_count {
            pagetable.unmap(start_vpn + i);
        }
        // TODO: preserve pages for future lazy cow
        inner.pages.clear();
        inner.status = SegmentStatus::Zombie;
        Ok(())
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::TrapContext
    }

    fn start_vpn(&self) -> VirtPageNum {
        TRAP_CONTEXT_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<Arc<dyn Segment>, ErrorNum> {
        Ok(Self::new(Some(self.clone())))
    }
}

impl IdenticalMappingSegment {
    pub fn new(range: VPNRange, flag: SegmentFlags) -> Arc<Self> {
        Arc::new(Self( SpinMutex::new("Segment lock", IdenticalMappingSegmentInner{
            range,
            flag,
            status: SegmentStatus::Initialized
        })))
    }
}

impl ManagedSegment {
    pub fn new(range: VPNRange, flag: SegmentFlags, clone_source: Option<Arc<Self>>) -> Arc<Self> {
        Arc::new(Self( SpinMutex::new("Segment lock", ManagedSegmentInner{
            range,
            frames: BTreeMap::new(),
            flag,
            status: SegmentStatus::Initialized,
            clone_source
        })))
    }

    pub fn alter_permission(&self, flag: SegmentFlags, pagetable: &mut PageTable) -> SegmentFlags {
        let mut inner = self.0.acquire();
        assert!(inner.status == SegmentStatus::Mapped, "altering unmapped segment flag");
        let original_flag = inner.flag;
        inner.flag = flag;
        for (vpn, pg) in inner.frames.iter() {
            let vpn = vpn.to_owned();
            let ppn = pg.ppn;
            pagetable.remap(vpn, ppn, flag.into());
        }
        original_flag
    }
}

impl VMASegment {
    pub fn new_at(start_vpn: VirtPageNum, file: Arc<dyn RegularFile>, flag: SegmentFlags) -> Arc<Self> {
        let res = VMASegmentInner {
            frames: BTreeMap::new(),
            file: file.clone(),
            flag,
            status: SegmentStatus::Initialized,
            start_vpn
        };
        Arc::new(VMASegment(SpinMutex::new("Segment lock", res)))
    }
}

impl TrampolineSegment {
    pub fn new() -> Arc<Self> {
        Arc::new(Self( SpinMutex::new("Segment lock", TrampolineSegmentInner{ status: SegmentStatus::Initialized } )))
    }
}

impl UTrampolineSegment {
    pub fn new() -> Arc<Self> {
        Arc::new(Self( SpinMutex::new("Segment lock", UTrampolineSegmentInner{ status: SegmentStatus::Initialized } )))
    }
}

impl TrapContextSegment {
    pub fn new(clone_source: Option<Arc<Self>>) -> Arc<Self> {
        Arc::new(Self(SpinMutex::new("Segment lock",  TrapContextSegmentInner{ status: SegmentStatus::Initialized, page: None, clone_source} )))
    }
}

impl ProcKStackSegment {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(SpinMutex::new("Segment lock", ProcKStackSegmentInner{ status: SegmentStatus::Initialized, pages: Vec::new()})))
    }
}

impl ProcUStackSegment {
    pub fn new(clone_source: Option<Arc<Self>>) -> Arc<Self> {
        Arc::new(Self(SpinMutex::new("Segment lock", ProcUStackSegmentInner{ status: SegmentStatus::Initialized, pages: Vec::new(), clone_source})))
    }
}