use core::fmt::{self, Debug, Formatter};

use core::cmp::min;
use core::ops::{Deref, DerefMut};
use alloc::string::String;
use alloc::{sync::{Arc}, collections::BTreeMap, vec::Vec, borrow::ToOwned};
use bitflags::*;
use crate::{config::{PAGE_SIZE, PROC_K_STACK_SIZE, PROC_K_STACK_ADDR, PROC_U_STACK_SIZE, PROC_U_STACK_ADDR}, utils::{SpinMutex, Mutex}};
use crate::{fs::{RegularFile}, utils::ErrorNum, config::{TRAMPOLINE_ADDR, U_TRAMPOLINE_ADDR, TRAP_CONTEXT_ADDR}};

use super::VirtAddr;
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
    fn contains(&self, vpn: VirtPageNum) -> bool;
    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum>;
}

pub struct ArcSegment(pub Arc<dyn Segment>);

/// All of these ArcSegment hassle, just to have PartialEq and Vec::contains... smh
impl PartialEq for ArcSegment {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

// no explicit Deref, use delegate functions instead.
// impl Deref for ArcSegment {
//     type Target = dyn Segment;

//     fn deref(&self) -> &Self::Target {
//         self.0.deref()
//     }
// }

impl From<Arc<dyn Segment>> for ArcSegment {
    fn from(s: Arc<dyn Segment>) -> Self {
        Self(s)
    }
}

impl Clone for ArcSegment {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl Debug for ArcSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// delegate functions.
impl ArcSegment {
    pub fn as_identical<'a>(self) -> Result<Arc<IdenticalMappingSegment>, ErrorNum> where Self: 'a {
        self.0.as_identical()
    }
    pub fn as_managed<'a>(self) -> Result<Arc<ManagedSegment>, ErrorNum> where Self: 'a{
        self.0.as_managed()
    }
    pub fn as_vma<'a>(self) -> Result<Arc<VMASegment>, ErrorNum> where Self: 'a{
        self.0.as_vma()
    }
    pub fn do_map(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum>{
        self.0.do_map(pagetable)
    }
    pub fn do_unmap(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum>{
        self.0.do_unmap(pagetable)
    }
    pub fn status(&self) -> SegmentStatus{
        self.0.status()
    }
    pub fn seg_type(&self) -> SegmentType{
        self.0.seg_type()
    }
    pub fn contains(&self, vpn: VirtPageNum) -> bool{
        self.0.contains(vpn)
    }
    pub fn clone_seg(&self) -> Result<ArcSegment, ErrorNum>{
        self.0.clone().clone_seg()
    }
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
    byte_len: usize,
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
    start_vpn: VirtPageNum,
    file_offset: usize,  /* file_offset in page */
    length: usize,  /* length in page */
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
        let inner = self.0.acquire();
        if inner.range.start() == inner.range.end() {return Ok(())}
        panic!("Parch don't unmap identitical mapping segment");
    }

    fn status(&self) -> SegmentStatus {
        self.0.acquire().status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::Identical
    }

    fn contains(&self, vpn: VirtPageNum) -> bool {
        self.0.acquire().range.contains(vpn)
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
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

    fn contains(&self, vpn: VirtPageNum) -> bool {
        self.0.acquire().frames.keys().any(|&x| x == vpn)
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
        let inner = self.0.acquire();
        Ok(Self::new(inner.range, inner.flag, Some(self.clone()), inner.byte_len))
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
        let length = min(inner.file.stat()?.file_size, inner.length);
        while offset < length {
            let pg = inner.file.get_page(offset + inner.file_offset)?;
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

    fn contains(&self, vpn: VirtPageNum) -> bool {
        self.0.acquire().frames.keys().any(|&x| x == vpn)
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
        let inner = self.0.acquire();
        Self::new_at(inner.start_vpn, inner.file.clone(), inner.flag, inner.file_offset, inner.length)
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

    fn contains(&self, vpn: VirtPageNum) -> bool {
        vpn == TRAMPOLINE_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
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

    fn contains(&self, vpn: VirtPageNum) -> bool {
        vpn == U_TRAMPOLINE_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
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

    fn contains(&self, vpn: VirtPageNum) -> bool {
        vpn == TRAP_CONTEXT_ADDR.into()
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
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

    fn contains(&self, vpn: VirtPageNum) -> bool {
        VPNRange::new(PROC_K_STACK_ADDR.into(), (PROC_K_STACK_ADDR + PROC_K_STACK_SIZE).into()).contains(vpn)
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
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

    fn contains(&self, vpn: VirtPageNum) -> bool {
        VPNRange::new(PROC_U_STACK_ADDR.into(), (PROC_U_STACK_ADDR + PROC_U_STACK_SIZE).into()).contains(vpn)
    }

    fn clone_seg(self: Arc<Self>) -> Result<ArcSegment, ErrorNum> {
        Ok(Self::new(Some(self.clone())))
    }
}

impl IdenticalMappingSegment {
    pub fn new(range: VPNRange, flag: SegmentFlags) -> ArcSegment {
        Arc::new(Self( SpinMutex::new("Segment lock", IdenticalMappingSegmentInner{
            range,
            flag,
            status: SegmentStatus::Initialized
        }))).as_segment().into()
    }
}

impl ManagedSegment {
    pub fn new(range: VPNRange, flag: SegmentFlags, clone_source: Option<Arc<ManagedSegment>>, byte_len: usize) -> ArcSegment {
        Arc::new(Self( SpinMutex::new("Segment lock", ManagedSegmentInner {
            range,
            byte_len,
            frames: BTreeMap::new(),
            flag,
            status: SegmentStatus::Initialized,
            clone_source
        }))).as_segment().into()
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

    pub fn grow(&self, increment: usize, pagetable: &mut PageTable) -> Result<VirtAddr, ErrorNum> {
        let mut inner = self.0.acquire();
        let mut map_iter = inner.range.end();
        let tgt_va: VirtAddr = VirtAddr::from(inner.range.start()) + inner.byte_len + increment;
        info!("Growing managed segment, original end {:?}, original length {}, increment {}", inner.range.end(), inner.byte_len, increment);
        while tgt_va.to_vpn_ceil() >= map_iter {
            debug!("Mapping vpn {:?}", map_iter);
            let pg = alloc_vm_page();
            let ppn = pg.ppn;
            pagetable.map(map_iter, ppn, inner.flag.into());
            inner.frames.insert(map_iter, pg);
            map_iter = map_iter + 1;
        }
        inner.byte_len = inner.byte_len + increment;
        inner.range.end = map_iter;
        info!("Grow done, new end {:?}, new length {}", inner.range.end(), inner.byte_len);
        Ok(VirtAddr::from(inner.range.start()) + inner.byte_len)
    }

    pub fn shrink(&self, decrement: usize, pagetable: &mut PageTable) -> Result<VirtAddr, ErrorNum> {
        let mut inner = self.0.acquire();
        let mut map_iter = inner.range.end() - 1;
        let tgt_va: VirtAddr = VirtAddr::from(inner.range.start()) + inner.byte_len - decrement;
        info!("Shrinking managed segment, original end {:?}, decrement {}", inner.range.end(), decrement);
        while tgt_va.to_vpn_ceil() < map_iter {
            debug!("Unapping vpn {:?}", map_iter);
            // remove pg
            inner.frames.remove(&map_iter).unwrap();
            pagetable.unmap(map_iter);
            map_iter = map_iter - 1;
        }
        inner.byte_len = inner.byte_len - decrement;
        inner.range.end = map_iter;
        info!("Grow done, new end {:?}, new length {}", inner.range.end(), inner.byte_len);
        Ok(VirtAddr::from(inner.range.start()) + inner.byte_len)

    }

    pub fn get_end_va(&self) -> VirtAddr {
        let inner = self.0.acquire();
        VirtAddr::from(inner.range.start()) + inner.byte_len
    }
}

impl VMASegment {
    /// file_offset and length are in bytes
    pub fn new_at(start_vpn: VirtPageNum, file: Arc<dyn RegularFile>, flag: SegmentFlags, file_offset: usize, length: usize) -> Result<ArcSegment, ErrorNum> {
        let res = VMASegmentInner {
            frames: BTreeMap::new(),
            file: file.clone(),
            flag,
            status: SegmentStatus::Initialized,
            start_vpn,
            file_offset,
            length,
        };
        Ok(Arc::new(VMASegment(SpinMutex::new("Segment lock", res))).as_segment().into())
    }
}

impl TrampolineSegment {
    pub fn new() -> ArcSegment {
        Arc::new(Self( SpinMutex::new("Segment lock", TrampolineSegmentInner{ status: SegmentStatus::Initialized } ))).as_segment().into()
    }
}

impl UTrampolineSegment {
    pub fn new() -> ArcSegment {
        Arc::new(Self( SpinMutex::new("Segment lock", UTrampolineSegmentInner{ status: SegmentStatus::Initialized } ))).as_segment().into()
    }
}

impl TrapContextSegment {
    pub fn new(clone_source: Option<Arc<TrapContextSegment>>) -> ArcSegment {
        Arc::new(Self(SpinMutex::new("Segment lock",  TrapContextSegmentInner{ status: SegmentStatus::Initialized, page: None, clone_source} ))).as_segment().into()
    }
}

impl ProcKStackSegment {
    pub fn new() -> ArcSegment {
        Arc::new(Self(SpinMutex::new("Segment lock", ProcKStackSegmentInner{ status: SegmentStatus::Initialized, pages: Vec::new()}))).as_segment().into()
    }
}

impl ProcUStackSegment {
    pub fn new(clone_source: Option<Arc<ProcUStackSegment>>) -> ArcSegment {
        Arc::new(Self(SpinMutex::new("Segment lock", ProcUStackSegmentInner{ status: SegmentStatus::Initialized, pages: Vec::new(), clone_source}))).as_segment().into()
    }
}