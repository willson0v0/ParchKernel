use core::fmt::{self, Debug, Formatter};

use alloc::{sync::{Arc}, collections::BTreeMap};
use bitflags::*;
use crate::{config::PAGE_SIZE, utils::{SpinMutex, Mutex}};
use crate::{fs::{RegularFile, File}, utils::ErrorNum, config::{TRAMPOLINE_ADDR, U_TRAMPOLINE_ADDR, TRAP_CONTEXT_ADDR}};
use crate::fs::OpenMode;
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
    fn do_map(&self, pagetable: &mut PageTable);
    fn do_unmap(&self, pagetable: &mut PageTable);
    fn status(&self) -> SegmentStatus;
    fn seg_type(&self) -> SegmentType;
    fn start_vpn(&self) -> VirtPageNum;
}

pub struct IdenticalMappingSegment (SpinMutex<IdenticalMappingSegmentInner>);

struct IdenticalMappingSegmentInner {
    range: VPNRange,
    flag: SegmentFlags,
    status: SegmentStatus
}

pub struct ManagedSegment (SpinMutex<ManagedSegmentInner>);
pub struct ManagedSegmentInner {
    range: VPNRange,
    frames: BTreeMap<VirtPageNum, PageGuard>,
    flag: SegmentFlags,
    status: SegmentStatus
}

pub struct VMASegment (SpinMutex<VMASegmentInner>);
pub struct VMASegmentInner {
    frames: BTreeMap<VirtPageNum, PageGuard>,
    file: Arc<dyn RegularFile>,
    flag: SegmentFlags,
    status: SegmentStatus,
    start_vpn: VirtPageNum,
}

pub struct TrampolineSegment (SpinMutex<TrampolineSegmentInner>);
pub struct TrampolineSegmentInner {
    status: SegmentStatus
}

pub struct UTrampolineSegment (SpinMutex<UTrampolineSegmentInner>);
pub struct UTrampolineSegmentInner {
    status: SegmentStatus
}

pub struct TrapContextSegment (SpinMutex<TrapContextSegmentInner>);
pub struct TrapContextSegmentInner {
    status: SegmentStatus,
    page: Option<PageGuard>
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
        f.write_fmt(format_args!("{:?} VMA segment with flag {:?}", inner.status, inner.flag))
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

    fn do_map(&self, pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return;
        }
        for vpn in inner.range {
            let ppn = PhysPageNum(vpn.0);
            pagetable.map(vpn, ppn, inner.flag.into())
        }
        inner.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) {
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

    fn do_map(&self, pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return;
        }
        for vpn in inner.range {
            let pageguard = alloc_vm_page();
            let ppn = pageguard.ppn;
            pagetable.map(vpn, ppn, inner.flag.into());
            inner.frames.insert(vpn, pageguard);
        }
        inner.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        assert!(inner.status == SegmentStatus::Mapped);
        for vpn in inner.range {
            inner.frames.remove(&vpn).unwrap();
        }
        inner.status = SegmentStatus::Zombie;
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

    fn do_map(&self, pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return;
        }
        
        for (vpn, pageguard) in &inner.frames {
            let ppn = pageguard.ppn;
            pagetable.map(*vpn, ppn, inner.flag.into());
        }

        inner.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&self, pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        assert!(inner.status == SegmentStatus::Mapped);
        for (vpn, _pg) in &inner.frames {
            pagetable.unmap(*vpn);
        }
        inner.frames.clear();
        inner.status = SegmentStatus::Zombie;
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

    fn do_map(&self, pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        extern "C" {
            fn strampoline();
        }
        
        if inner.status != SegmentStatus::Initialized {
            return;
        }
        pagetable.map(
            TRAMPOLINE_ADDR.into(),
            PhysAddr::from(strampoline as usize).into(), 
            PTEFlags::R | PTEFlags::X
        );
        inner.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) {
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

    fn do_map(&self, pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        extern "C" {
            fn sutrampoline();
        }
        
        if inner.status != SegmentStatus::Initialized {
            return;
        }
        pagetable.map(
            U_TRAMPOLINE_ADDR.into(),
            PhysAddr::from(sutrampoline as usize).into(), 
            PTEFlags::R | PTEFlags::X | PTEFlags::U
        );
        inner.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) {
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

    fn do_map(&self, pagetable: &mut PageTable) {
        let mut inner = self.0.acquire();
        if inner.status != SegmentStatus::Initialized {
            return;
        }
        let pageguard = alloc_vm_page();
        let ppn = pageguard.ppn;
        pagetable.map(
            TRAP_CONTEXT_ADDR.into(),
            ppn, 
            PTEFlags::R | PTEFlags::W
        );
        inner.status = SegmentStatus::Mapped;
        inner.page = Some(pageguard);
    }

    fn do_unmap(&self, _pagetable: &mut PageTable) {
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
    pub fn new(range: VPNRange, flag: SegmentFlags) -> Arc<Self> {
        Arc::new(Self( SpinMutex::new("Segment lock", ManagedSegmentInner{
            range,
            frames: BTreeMap::new(),
            flag,
            status: SegmentStatus::Initialized
        })))
    }
}

impl VMASegment {
    pub fn new_at(start_vpn: VirtPageNum, len: usize, mut offset: usize, file: Arc<dyn RegularFile>, flag: SegmentFlags) -> Result< Arc<Self>, ErrorNum> {
        let stat = file.stat()?;
        if !stat.open_mode.contains(OpenMode::SYS) {
            if !stat.open_mode.contains(OpenMode::WRITE) && flag.contains(SegmentFlags::W) {
                return Err(ErrorNum::EPERM);
            }
        }
        if stat.file_size == 0 {
            return Err(ErrorNum::EEMPTY)
        }
        if offset+len > stat.file_size {
            return Err(ErrorNum::EOOR);
        }

        let mut res = VMASegmentInner {
            frames: BTreeMap::new(),
            file: file.clone(),
            flag,
            status: SegmentStatus::Initialized,
            start_vpn
        };
        let mut vpn = start_vpn;
        while offset + len < stat.file_size {
            let pg = file.get_page(offset)?;
            res.frames.insert(vpn, pg);
            offset += PAGE_SIZE;
            vpn += 1;
        }
        Ok(Arc::new(VMASegment(SpinMutex::new("Segment lock", res))))
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
    pub fn new() -> Arc<Self> {
        Arc::new(Self(SpinMutex::new("Segment lock",  TrapContextSegmentInner{ status: SegmentStatus::Initialized, page: None} )))
    }
}