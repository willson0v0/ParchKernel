use core::fmt::{self, Debug, Formatter};
use alloc::{sync::{Arc}, collections::BTreeMap, boxed::Box};
use bitflags::*;
use crate::{fs::{File, RegularFile}, utils::ErrorNum, config::{TRAMPOLINE_ADDR, U_TRAMPOLINE_ADDR, TRAP_CONTEXT_ADDR}};

use super::{types::{VPNRange, VirtPageNum, PhysPageNum}, PageGuard, pagetable::{PageTable, PTEFlags}, alloc_page, PhysAddr};


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


pub trait Segment: Debug {
    fn as_segment   <'a>(self: Arc<Self>) -> Arc<dyn Segment + 'a> where Self: 'a;
    fn as_identical <'a>(self: Arc<Self>) -> Result<Arc<IdenticalMappingSegment >, ErrorNum> where Self: 'a;
    fn as_managed   <'a>(self: Arc<Self>) -> Result<Arc<ManagedSegment          >, ErrorNum> where Self: 'a;
    fn as_vma       <'a>(self: Arc<Self>) -> Result<Arc<VMASegment              >, ErrorNum> where Self: 'a;
    fn do_map(&mut self, pagetable: &mut PageTable);
    fn do_unmap(&mut self, pagetable: &mut PageTable);
    fn status(&self) -> SegmentStatus;
    fn seg_type(&self) -> SegmentType;
}

pub struct IdenticalMappingSegment {
    range: VPNRange,
    flag: SegmentFlags,
    status: SegmentStatus
}

pub struct ManagedSegment {
    range: VPNRange,
    frames: BTreeMap<VirtPageNum, PageGuard>,
    flag: SegmentFlags,
    status: SegmentStatus
}

pub struct VMASegment {
    frames: BTreeMap<VirtPageNum, PageGuard>,
    file: Arc<dyn RegularFile>,
    flag: SegmentFlags,
    status: SegmentStatus
}

pub struct TrampolineSegment {
    status: SegmentStatus
}

pub struct UTrampolineSegment {
    status: SegmentStatus
}

pub struct TrapContextSegment {
    status: SegmentStatus,
    page: Option<PageGuard>
}

impl Debug for IdenticalMappingSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:?} Identical segment {:?} ~ {:?} with flag {:?}", self.status, self.range.start(), self.range.end(), self.flag))
    }
}

impl Debug for ManagedSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:?} Managed segment {:?} ~ {:?} with flag {:?}", self.status, self.range.start(), self.range.end(), self.flag))
    }
}

impl Debug for VMASegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // TODO: Add file desc
        f.write_fmt(format_args!("{:?} VMA segment with flag {:?}", self.status, self.flag))
    }
}

impl Debug for TrampolineSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:?} Trampoline segment", self.status))
    }
}

impl Debug for UTrampolineSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:?} UTrampoline segment", self.status))
    }
}

impl Debug for TrapContextSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:?} TrapContext segment", self.status))
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

    fn do_map(&mut self, pagetable: &mut PageTable) {
        assert!(self.status() == SegmentStatus::Initialized);
        for vpn in self.range {
            let ppn = PhysPageNum(vpn.0);
            pagetable.map(vpn, ppn, self.flag.into())
        }
        self.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&mut self, pagetable: &mut PageTable) {
        panic!("Parch don't unmap identitical mapping segment");
    }

    fn status(&self) -> SegmentStatus {
        self.status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::Identical
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

    fn do_map(&mut self, pagetable: &mut PageTable) {
        assert!(self.status() == SegmentStatus::Initialized);
        for vpn in self.range {
            let pageguard = alloc_page(true);
            let ppn = pageguard.ppn;
            pagetable.map(vpn, ppn, self.flag.into());
            self.frames.insert(vpn, pageguard);
        }
        self.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&mut self, pagetable: &mut PageTable) {
        assert!(self.status() == SegmentStatus::Mapped);
        for vpn in self.range {
            self.frames.remove(&vpn).unwrap();
        }
        self.status = SegmentStatus::Zombie;
    }

    fn status(&self) -> SegmentStatus {
        self.status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::Managed
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

    fn do_map(&mut self, pagetable: &mut PageTable) {
        assert!(self.status() == SegmentStatus::Initialized);
        self.status = SegmentStatus::Mapped;
        todo!();
    }

    fn do_unmap(&mut self, pagetable: &mut PageTable) {
        assert!(self.status() == SegmentStatus::Mapped);
        self.status = SegmentStatus::Zombie;
        todo!()
    }

    fn status(&self) -> SegmentStatus {
        self.status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::VMA
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

    fn do_map(&mut self, pagetable: &mut PageTable) {
        extern "C" {
            fn strampoline();
        }
        assert!(self.status() == SegmentStatus::Initialized);
        pagetable.map(
            U_TRAMPOLINE_ADDR.into(),
            PhysAddr::from(strampoline as usize).into(), 
            PTEFlags::R | PTEFlags::X
        );
        self.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&mut self, pagetable: &mut PageTable) {
        panic!("Don't unmap trampoline!")
    }

    fn status(&self) -> SegmentStatus {
        self.status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::Trampoline
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

    fn do_map(&mut self, pagetable: &mut PageTable) {
        extern "C" {
            fn sutrampoline();
        }
        assert!(self.status() == SegmentStatus::Initialized);
        pagetable.map(
            TRAMPOLINE_ADDR.into(),
            PhysAddr::from(sutrampoline as usize).into(), 
            PTEFlags::R | PTEFlags::X | PTEFlags::U
        );
        self.status = SegmentStatus::Mapped;
    }

    fn do_unmap(&mut self, pagetable: &mut PageTable) {
        panic!("Don't unmap u_trampoline!")
    }

    fn status(&self) -> SegmentStatus {
        self.status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::UTrampoline
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

    fn do_map(&mut self, pagetable: &mut PageTable) {
        assert!(self.status() == SegmentStatus::Initialized);
        let pageguard = alloc_page(true);
        let ppn = pageguard.ppn;
        pagetable.map(
            TRAP_CONTEXT_ADDR.into(),
            ppn, 
            PTEFlags::R | PTEFlags::W
        );
        self.status = SegmentStatus::Mapped;
        self.page = Some(pageguard);
    }

    fn do_unmap(&mut self, pagetable: &mut PageTable) {
        panic!("Don't unmap trap_context!")
    }

    fn status(&self) -> SegmentStatus {
        self.status
    }

    fn seg_type(&self) -> SegmentType {
        SegmentType::TrapContext
    }
}

impl IdenticalMappingSegment {
    pub fn new(range: VPNRange, flag: SegmentFlags) -> Box<Self> {
        Box::new(Self {
            range,
            flag,
            status: SegmentStatus::Initialized
        })
    }
}

impl ManagedSegment {
    pub fn new(range: VPNRange, flag: SegmentFlags) -> Box<Self> {
        Box::new(Self {
            range,
            frames: BTreeMap::new(),
            flag,
            status: SegmentStatus::Initialized
        })
    }
}

impl VMASegment {
    pub fn new() -> Box<Self> {
        todo!()
    }
}

impl TrampolineSegment {
    pub fn new() -> Box<Self> {
        Box::new(Self { status: SegmentStatus::Initialized })
    }
}

impl UTrampolineSegment {
    pub fn new() -> Box<Self> {
        Box::new(Self { status: SegmentStatus::Initialized })
    }
}

impl TrapContextSegment {
    pub fn new() -> Box<Self> {
        Box::new(Self { status: SegmentStatus::Initialized, page: None })
    }
}