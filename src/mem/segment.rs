use core::fmt::{self, Debug, Formatter};

use core::cmp::min;


use alloc::{sync::{Arc}, collections::BTreeMap, vec::Vec, borrow::ToOwned};
use bitflags::*;
use crate::fs::File;
use crate::{config::{PAGE_SIZE, PROC_K_STACK_SIZE, PROC_K_STACK_ADDR, PROC_U_STACK_SIZE, PROC_U_STACK_ADDR}, utils::{SpinMutex, Mutex}};
use crate::{fs::{RegularFile}, utils::ErrorNum, config::{TRAMPOLINE_ADDR, U_TRAMPOLINE_ADDR, TRAP_CONTEXT_ADDR}};

use super::{VirtAddr, PageTableEntry};
use super::{types::{VPNRange, VirtPageNum, PhysPageNum}, PageGuard, pagetable::{PageTable, PTEFlags}, alloc_vm_page, PhysAddr};


// TODO: assert !w on all cow

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

#[derive(Clone, Debug)]
pub enum PageGuardSlot {
    Unmapped,
    LazyAlloc,
    Populated(PageGuard),
    CopyOnWrite(PageGuard),
    LazyVMA((Arc<dyn RegularFile>, usize)),    // file & offset // TODO: change this to Arc<dyn File>, for we might be able to mmap device file.
}

impl PageGuardSlot {
    /// Returns `true` if the page guard slot is [`Unmapped`].
    ///
    /// [`Unmapped`]: PageGuardSlot::Unmapped
    pub fn is_unmapped(&self) -> bool {
        matches!(self, Self::Unmapped)
    }

    /// Returns `true` if the page guard slot is [`Lazy`].
    ///
    /// [`Lazy`]: PageGuardSlot::Lazy
    pub fn is_lazy(&self) -> bool {
        matches!(self, Self::LazyAlloc)
    }
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
    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum>;
    fn do_lazy(&self, vpn: VirtPageNum, pagetable: &mut PageTable) -> Result<(), ErrorNum>;
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
    pub fn clone_seg(&self, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum>{
        self.0.clone().clone_seg(pagetable)
    }
    pub fn do_lazy(&self, vpn: VirtPageNum, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        self.0.do_lazy(vpn, pagetable)
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
    pub range: VPNRange,
    pub byte_len: usize,
    pub frames: BTreeMap<VirtPageNum, PageGuardSlot>,
    pub flag: SegmentFlags,
    pub status: SegmentStatus,
}

pub struct VMASegment (SpinMutex<VMASegmentInner>);
pub struct VMASegmentInner {
    frames: BTreeMap<VirtPageNum, PageGuardSlot>,
    // file: Arc<dyn RegularFile>,
    flag: SegmentFlags,
    status: SegmentStatus,
    start_vpn: VirtPageNum,
    // file_offset: usize,  /* file_offset in page */
    // length: usize,  /* length in page */
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
    pub frames: BTreeMap<VirtPageNum, PageGuardSlot>,
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
        f.write_fmt(format_args!("{:?} VMA segment of {} frames @ {:?} with flag {:?}", inner.status, inner.frames.len(), inner.start_vpn, inner.flag))
    }
}

impl Debug for TrampolineSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} Trampoline segment @ {:?}", inner.status, TRAMPOLINE_ADDR))
    }
}

impl Debug for UTrampolineSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} UTrampoline segment @ {:?}", inner.status, U_TRAMPOLINE_ADDR))
    }
}

impl Debug for TrapContextSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} TrapContext segment @ {:?}", inner.status, TRAP_CONTEXT_ADDR))
    }
}

impl Debug for ProcKStackSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} ProcKStack segment @ {:?} ~ {:?}", inner.status, PROC_K_STACK_ADDR, PROC_K_STACK_ADDR + PROC_K_STACK_SIZE))
    }
}

impl Debug for ProcUStackSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let inner = self.0.acquire();
        f.write_fmt(format_args!("{:?} ProcUStack segment @ {:?} ~ {:?}", inner.status, PROC_U_STACK_ADDR, PROC_U_STACK_ADDR + PROC_U_STACK_SIZE))
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

    fn clone_seg(self: Arc<Self>, _pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        let inner = self.0.acquire();
        Ok(Self::new(inner.range, inner.flag))
    }

    fn do_lazy(&self, vpn: VirtPageNum, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let inner = self.0.acquire();
        if inner.range.contains(vpn) {
            let ppn = PhysPageNum(vpn.0);
            pagetable.map(vpn, ppn, inner.flag.into());
            Ok(())
        } else {
            Err(ErrorNum::EOOR)
        }
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

        for (vpn, pgs) in inner.frames.iter() {
            if let PageGuardSlot::CopyOnWrite(pg) = pgs {
                pagetable.map(*vpn, pg.ppn, (inner.flag & SegmentFlags::W.complement()).into());
            }
        }
        inner.status = SegmentStatus::Mapped;

        Ok(())
    }

    fn do_unmap(&self, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        assert!(inner.status == SegmentStatus::Mapped);
        for vpn in inner.range {
            // not dropping pageguards, for lazy cow.
            // inner.frames.remove(&vpn).unwrap();
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

    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        let mut inner = self.0.acquire();

        let new_frames: BTreeMap<VirtPageNum, PageGuardSlot> = inner.frames.iter().map(|(vpn, slot)| -> (VirtPageNum, PageGuardSlot) {
            let new_slot = match slot {
                PageGuardSlot::Unmapped => panic!("cannot unmap partly in managed."),
                PageGuardSlot::LazyAlloc => PageGuardSlot::LazyAlloc,
                PageGuardSlot::Populated(content) => {
                    pagetable.remap(*vpn, content.ppn, (inner.flag & SegmentFlags::W.complement()).into()); // disable write to trigger cow
                    PageGuardSlot::CopyOnWrite(content.clone())
                },
                PageGuardSlot::CopyOnWrite(content) => PageGuardSlot::CopyOnWrite(content.clone()),
                PageGuardSlot::LazyVMA(_) => panic!("no vma in managed."),
            };
            (*vpn, new_slot)
        }).collect();

        inner.frames = new_frames.clone();

        let res = Self (SpinMutex::new("segment", ManagedSegmentInner { 
            range: inner.range,
            byte_len: inner.byte_len,
            frames: new_frames,
            flag: inner.flag,
            status: SegmentStatus::Initialized,
        }));

        Ok(Arc::new(res).as_segment().into())
    }

    fn do_lazy(&self, vpn: VirtPageNum, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();

        if inner.range.contains(vpn) {
            let pageslot = inner.frames.get(&vpn).cloned().unwrap();
            if let PageGuardSlot::CopyOnWrite(cow_source) = pageslot {
                if !inner.flag.contains(SegmentFlags::W) {
                    // real pagefault
                    return Err(ErrorNum::EPERM)
                }

                // one here, one remain in frames
                let tgt_page = if Arc::strong_count(&cow_source) == 2 {
                    verbose!("Only one refrence left on cow page, not copying.");
                    inner.frames.insert(vpn, PageGuardSlot::Populated(cow_source.clone()));
                    cow_source
                } else {
                    verbose!("COW triggered.");
                    let pageguard = alloc_vm_page();
                    unsafe {PhysPageNum::copy_page(&cow_source.ppn, &pageguard.ppn)}
                    inner.frames.insert(vpn, PageGuardSlot::Populated(pageguard.clone()));
                    pageguard
                };
                pagetable.remap(vpn, tgt_page.ppn, inner.flag.into())
            } else if let PageGuardSlot::LazyAlloc = pageslot {
                verbose!("Lazy alloc triggered.");
                let pageguard = alloc_vm_page();
                let ppn = pageguard.ppn;
                pagetable.map(vpn, ppn, inner.flag.into());
                inner.frames.insert(vpn, PageGuardSlot::Populated(pageguard));
            } else if let PageGuardSlot::Populated(_) = pageslot {
                verbose!("real pagefault.");
                return Err(ErrorNum::EPERM);
            } else {
                panic!("No VMA in managed segement.");
            }
            Ok(())
        } else {
            Err(ErrorNum::EOOR)
        }
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

        for (vpn, pgs) in inner.frames.iter() {
            if let PageGuardSlot::CopyOnWrite(pg) = pgs {
                pagetable.map(*vpn, pg.ppn, (inner.flag & SegmentFlags::W.complement()).into());
            }
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

    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        let mut inner = self.0.acquire();
    
        let new_frames: BTreeMap<VirtPageNum, PageGuardSlot> = inner.frames.iter().map(|(vpn, slot)| -> (VirtPageNum, PageGuardSlot) {
            let new_slot = match slot {
                PageGuardSlot::CopyOnWrite(content) => PageGuardSlot::CopyOnWrite(content.clone()),
                PageGuardSlot::Populated(content) => {
                    pagetable.remap(*vpn, content.ppn, (inner.flag & SegmentFlags::W.complement()).into()); // disable write to trigger cow
                    PageGuardSlot::CopyOnWrite(content.clone())
                },
                PageGuardSlot::LazyVMA((file, offset)) =>  PageGuardSlot::LazyVMA((file.clone(), *offset)),
                PageGuardSlot::LazyAlloc =>  PageGuardSlot::LazyAlloc,
                _ => panic!("Bad slot type in vma")
            };

            (*vpn, new_slot)
        }).collect();

        inner.frames = new_frames.clone();

        let res = Self (SpinMutex::new("segment", VMASegmentInner {
            frames: new_frames,
            flag: inner.flag,
            status: SegmentStatus::Initialized,
            start_vpn: inner.start_vpn,
        }));

        Ok(Arc::new(res).as_segment().into())
    }

    fn do_lazy(&self, vpn: VirtPageNum, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();

        if inner.frames.contains_key(&vpn) {
            let pageslot = inner.frames.get(&vpn).cloned().unwrap();

            match pageslot {
                PageGuardSlot::Unmapped => return Err(ErrorNum::EPERM), // was unmapped
                PageGuardSlot::LazyAlloc => {
                    verbose!("lazy alloc triggered.");
                    let pg = alloc_vm_page();
                    inner.frames.insert(vpn, PageGuardSlot::Populated(pg.clone()));
                    pagetable.map(vpn, pg.ppn, inner.flag.into())
                },
                PageGuardSlot::Populated(_) => return Err(ErrorNum::EPERM), // real pagefault
                PageGuardSlot::CopyOnWrite(content) => {
                    if !inner.flag.contains(SegmentFlags::W) {
                        // real pagefault
                        return Err(ErrorNum::EPERM)
                    }
    
                    debug_assert!(inner.flag.contains(SegmentFlags::R) && inner.flag.contains(SegmentFlags::W), "lazy bad seg");
    
                    // one here, one remain in frames
                    // no data race here, for this segment was locked and content will not be copied,
                    // and there are no other segment holding such content.
                    let tgt_page = if Arc::strong_count(&content) == 2 {
                        verbose!("Only one refrence left on cow page, not copying.");
                        inner.frames.insert(vpn, PageGuardSlot::Populated(content.clone()));
                        content
                    } else {
                        verbose!("COW triggered.");
                        let pageguard = alloc_vm_page();
                        unsafe {PhysPageNum::copy_page(&content.ppn, &pageguard.ppn)}
                        inner.frames.insert(vpn, PageGuardSlot::Populated(pageguard.clone()));
                        pageguard
                    };
                    pagetable.remap(vpn, tgt_page.ppn, inner.flag.into())
                },
                PageGuardSlot::LazyVMA((file, offset)) => {
                    verbose!("lazy vma triggered.");
                    let pg = file.get_page(offset)?;
                    pagetable.map(vpn, pg.ppn, inner.flag.into());
                    inner.frames.insert(vpn, PageGuardSlot::Populated(pg));
                },
            }
            Ok(())
        } else {
            Err(ErrorNum::EOOR)
        }
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

    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        Ok(Self::new())
    }

    fn do_lazy(&self, vpn: VirtPageNum, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        if vpn == TRAMPOLINE_ADDR.into() {
            Err(ErrorNum::EPERM)
        } else {
            Err(ErrorNum::EOOR)
        }
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

    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        Ok(Self::new())
    }

    fn do_lazy(&self, vpn: VirtPageNum, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        if vpn == U_TRAMPOLINE_ADDR.into() {
            Err(ErrorNum::EPERM)
        } else {
            Err(ErrorNum::EOOR)
        }
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

    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        Ok(Self::new(Some(self.clone())))
    }

    fn do_lazy(&self, vpn: VirtPageNum, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        if vpn == TRAP_CONTEXT_ADDR.into() {
            Err(ErrorNum::EPERM)
        } else {
            Err(ErrorNum::EOOR)
        }
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

    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        Ok(Self::new())
    }

    fn do_lazy(&self, vpn: VirtPageNum, _pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        if VPNRange::new(PROC_K_STACK_ADDR.into(), (PROC_K_STACK_ADDR + PROC_K_STACK_SIZE).into()).contains(vpn) {
            Err(ErrorNum::EPERM)
        } else {
            Err(ErrorNum::EOOR)
        }
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

        for (vpn, pgs) in inner.frames.iter() {
            if let PageGuardSlot::CopyOnWrite(pg) = pgs {
                pagetable.map(*vpn, pg.ppn, PTEFlags::R | PTEFlags::U);
            }
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
        inner.frames = inner.frames.iter().map(|(vpn, slot)| -> (VirtPageNum, PageGuardSlot) {
            (*vpn, PageGuardSlot::Unmapped)
        }).collect();
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

    fn clone_seg(self: Arc<Self>, pagetable: &mut PageTable) -> Result<ArcSegment, ErrorNum> {
        let mut inner = self.0.acquire();

        let frames: BTreeMap<VirtPageNum, PageGuardSlot> = inner.frames.iter().map(|(vpn, pgs)| -> (VirtPageNum, PageGuardSlot) {
            match pgs {
                PageGuardSlot::LazyAlloc => (*vpn, PageGuardSlot::LazyAlloc),
                PageGuardSlot::Populated(content) => {
                    verbose!("Remapping u stack clone source {:?} to non writable", *vpn);
                    pagetable.remap(*vpn, content.ppn, PTEFlags::R | PTEFlags::U);
                    (*vpn, PageGuardSlot::CopyOnWrite(content.clone()))
                },
                PageGuardSlot::CopyOnWrite(content) => (*vpn, PageGuardSlot::CopyOnWrite(content.clone())),
                _ => panic!("bad map type"),
            }
        }).collect();
        inner.frames = frames.clone();
        Ok(Arc::new(Self(SpinMutex::new("segment", ProcUStackSegmentInner{
            status: SegmentStatus::Initialized,
            frames,
        }))).as_segment().into())
        // Ok(Self::new(Some(self.clone())))
    }

    fn do_lazy(&self, vpn: VirtPageNum, pagetable: &mut PageTable) -> Result<(), ErrorNum> {
        let mut inner = self.0.acquire();
        if  let Some(pageslot) = inner.frames.get(&vpn).cloned() {
            match pageslot.clone() {
                PageGuardSlot::Unmapped => panic!("unmapped proc u stack"),
                PageGuardSlot::LazyAlloc => {
                    verbose!("Lazy alloc triggered.");
                    let pageguard = alloc_vm_page();
                    let ppn = pageguard.ppn;
                    pagetable.map(vpn, ppn, PTEFlags::R | PTEFlags::W | PTEFlags::U);
                    inner.frames.insert(vpn, PageGuardSlot::Populated(pageguard));
                },
                PageGuardSlot::Populated(_) => return {
                    let pte: PageTableEntry = unsafe{pagetable.walk_find(vpn).unwrap().read_volatile()};
                    error!("Populated lazy triggered for Proc U stack. wut? flag {:?}", pte.flags());
                    Err(ErrorNum::EPERM)
                },
                PageGuardSlot::CopyOnWrite(cow_source) => {
                    // debug_assert!(inner.flag.contains(SegmentFlags::R) && inner.flag.contains(SegmentFlags::W), "lazy bad seg");
    
                    // one here, one remain in frames
                    // no data race here, for this segment was locked and content will not be copied,
                    // and there are no other segment holding such content.
                    let tgt_page = if Arc::strong_count(&cow_source) == 2 {
                        verbose!("Only one refrence left on cow page, not copying.");
                        inner.frames.insert(vpn, PageGuardSlot::Populated(cow_source.clone()));
                        cow_source
                    } else {
                        verbose!("COW triggered.");
                        let pageguard = alloc_vm_page();
                        unsafe {PhysPageNum::copy_page(&cow_source.ppn, &pageguard.ppn)}
                        inner.frames.insert(vpn, PageGuardSlot::Populated(pageguard.clone()));
                        pageguard
                    };
                    pagetable.do_map(vpn, tgt_page.ppn, PTEFlags::R | PTEFlags::W | PTEFlags::U);
                },
                PageGuardSlot::LazyVMA(_) => panic!("lazy vma in proc u stack"),
            }
            Ok(())
        } else {
            Err(ErrorNum::EOOR)
        }
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
    pub fn new(range: VPNRange, flag: SegmentFlags, byte_len: usize) -> ArcSegment {
        let frames: BTreeMap<VirtPageNum, PageGuardSlot> = range.clone().into_iter().map(|vpn| (vpn, PageGuardSlot::LazyAlloc)).collect();
        Arc::new(Self( SpinMutex::new("Segment lock", ManagedSegmentInner {
            range,
            byte_len,
            frames,
            flag,
            status: SegmentStatus::Initialized
        }))).as_segment().into()
    }

    pub fn alter_permission(&self, flag: SegmentFlags, pagetable: &mut PageTable) -> SegmentFlags {
        let mut inner = self.0.acquire();
        assert!(inner.status == SegmentStatus::Mapped, "altering bad segment's flag");
        let original_flag = inner.flag;
        inner.flag = flag;
        for (vpn, slot) in inner.frames.iter() {
            let vpn = vpn.to_owned();
            let slot = slot.to_owned();
            match slot {
                PageGuardSlot::LazyAlloc => {/* do nothing. */},
                PageGuardSlot::Populated(pg) => {
                    let ppn = pg.ppn;
                    pagetable.remap(vpn, ppn, flag.into());
                },
                PageGuardSlot::CopyOnWrite(_) => {/* do nothing */},
                _ => panic!("bad slot type")
            }
        }
        original_flag
    }

    pub fn grow(&self, increment: usize, pagetable: &mut PageTable) -> Result<VirtAddr, ErrorNum> {
        let mut inner = self.0.acquire();
        let mut map_iter = inner.range.end();
        let tgt_va: VirtAddr = VirtAddr::from(inner.range.start()) + inner.byte_len + increment;
        info!("Growing managed segment, original end {:?}, original length {}, increment {}", inner.range.end(), inner.byte_len, increment);
        while tgt_va.to_vpn_ceil() >= map_iter {
            debug!("registering lazy vpn {:?}", map_iter);
            // let pg = alloc_vm_page();
            // let ppn = pg.ppn;
            // pagetable.map(map_iter, ppn, inner.flag.into());
            inner.frames.insert(map_iter, PageGuardSlot::LazyAlloc);
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
            let slot = inner.frames.remove(&map_iter).unwrap();
            match slot {
                PageGuardSlot::LazyAlloc => { /* do nothing */ },
                PageGuardSlot::Populated(_) => {
                    pagetable.unmap(map_iter);
                },
                PageGuardSlot::CopyOnWrite(_) => {
                    pagetable.unmap(map_iter);
                },
                _ => panic!("bad slot type"),
            }
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
        let file_size = file.stat()?.file_size;
        let frames = VPNRange::new(
            start_vpn, 
            (VirtAddr::from(start_vpn) + length).to_vpn_ceil()
        )
            .into_iter()
            .map(|vpn| -> (VirtPageNum, PageGuardSlot) {
                let offset_to_file = file_offset + (vpn - start_vpn) * PAGE_SIZE;
                (vpn, PageGuardSlot::LazyVMA((file.clone(), offset_to_file)))
            })
            .collect();
        let res = VMASegmentInner {
            frames,
            flag,
            status: SegmentStatus::Initialized,
            start_vpn,
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
    pub fn new() -> ArcSegment {
        let start_vpn = VirtPageNum::from(PROC_U_STACK_ADDR);
        let end_vpn = VirtPageNum::from(PROC_U_STACK_ADDR + PROC_U_STACK_SIZE);
        let frames: BTreeMap<VirtPageNum, PageGuardSlot> = VPNRange::new(start_vpn, end_vpn)
            .into_iter()
            .map(|vpn| -> (VirtPageNum, PageGuardSlot) {
                (vpn, PageGuardSlot::LazyAlloc)
            })
            .collect();
        Arc::new(Self(SpinMutex::new("Segment lock", ProcUStackSegmentInner{ status: SegmentStatus::Initialized, frames}))).as_segment().into()
    }
}