
use core::fmt::{self, Debug, Formatter};
use core::slice::from_raw_parts_mut;
use core::{ops};
use core::ptr::{read_volatile, write_volatile, copy_nonoverlapping};


use alloc::vec::Vec;

use crate::config::{PAGE_OFFSET, PAGE_SIZE};
use crate::utils::range::{StepUp, StepDown, Range};

#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);


#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);


/// The representation of physical page number.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysPageNum(pub usize);

/// The representation of virtual page number.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtPageNum(pub usize);


impl Debug for VirtAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VA<{:#x}>", self.0))
    }
}
impl Debug for VirtPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("VPN<{:#x}>", self.0))
    }
}
impl Debug for PhysAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PA<{:#x}>", self.0))
    }
}
impl Debug for PhysPageNum {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("PPN<{:#x}>", self.0))
    }
}

impl From<usize> for PhysAddr       { fn from(num: usize) -> Self { Self(num) } }
impl From<usize> for VirtAddr       { fn from(num: usize) -> Self { Self(num) } }
impl From<usize> for VirtPageNum    { fn from(num: usize) -> Self { Self(num) } }
impl From<usize> for PhysPageNum    { fn from(num: usize) -> Self { Self(num) } }

impl ops::Add<usize> for VirtAddr {
    type Output = VirtAddr;
    fn add(self, rhs: usize) -> VirtAddr {
        return VirtAddr(self.0 + rhs);
    }
}

impl ops::AddAssign<usize> for VirtAddr {
    fn add_assign(&mut self, rhs: usize) { 
        self.0 += rhs;
    }
}
impl ops::Sub<usize> for VirtAddr {
    type Output = VirtAddr;
    fn sub(self, rhs: usize) -> VirtAddr {
        return VirtAddr(self.0 - rhs);
    }
}

impl ops::Sub<VirtAddr> for VirtAddr {
    type Output = usize;
    fn sub(self, rhs: VirtAddr) -> usize {
        return self.0 - rhs.0;
    }
}

impl ops::SubAssign<usize> for VirtAddr {
    fn sub_assign(&mut self, rhs: usize) { 
        self.0 -= rhs;
    }
}

// TODO: SV39 out of bound detection
impl ops::Add<usize> for PhysAddr {
    type Output = PhysAddr;
    fn add(self, rhs: usize) -> PhysAddr {
        return PhysAddr(self.0 + rhs);
    }
}

impl ops::AddAssign<usize> for PhysAddr {
    fn add_assign(&mut self, rhs: usize) { 
        self.0 += rhs;
    }
}
impl ops::Sub<usize> for PhysAddr {
    type Output = PhysAddr;
    fn sub(self, rhs: usize) -> PhysAddr {
        return PhysAddr(self.0 - rhs);
    }
}

impl ops::Sub<PhysAddr> for PhysAddr {
    type Output = usize;
    fn sub(self, rhs: PhysAddr) -> usize {
        return self.0 - rhs.0;
    }
}

impl ops::SubAssign<usize> for PhysAddr {
    fn sub_assign(&mut self, rhs: usize) { 
        self.0 -= rhs;
    }
}

impl PhysAddr {
    pub unsafe fn write_volatile<T: Clone>(&self, data: &T) {
        write_volatile(self.0 as *mut T, data.clone());
    }

    pub unsafe fn read_volatile<T: Sized>(&self) -> T {
        read_volatile(self.0 as *const T)
    }

    pub unsafe fn instantiate_volatile<T>(&self) -> &'static mut T {
        (self.0 as *mut T).as_mut().unwrap()
    }

    pub unsafe fn write_data(&self, data: Vec<u8>) {
        copy_nonoverlapping(data.as_ptr(), self.0 as * mut u8, data.len());
    }

    pub unsafe fn read_data(&self, length: usize) -> Vec<u8> {
        from_raw_parts_mut(self.0 as *mut u8, length).to_vec()
    }

    pub fn to_ppn_ceil(&self) -> PhysPageNum {
        if self.0 == 0 {
            1.into()
        } else {
            PhysPageNum(((self.0 - 1) >> PAGE_OFFSET) + 1)
        }
    }
}

impl VirtAddr {
    pub unsafe fn write_volatile<T: Clone>(&self, data: &T) {
        write_volatile(self.0 as *mut T, data.clone());
    }

    pub unsafe fn read_volatile<T: Sized>(&self) -> T {
        read_volatile(self.0 as *const T)
    }

    pub unsafe fn instantiate_volatile<T>(&self) -> &'static mut T {
        (self.0 as *mut T).as_mut().unwrap()
    }

    pub unsafe fn write_data(&self, data: Vec<u8>) {
        copy_nonoverlapping(data.as_ptr(), self.0 as * mut u8, data.len());
    }

    /// This WILL copy the data (to_vec did it)
    pub unsafe fn read_data(&self, length: usize) -> Vec<u8> {
        from_raw_parts_mut(self.0 as *mut u8, length).to_vec()
    }

    pub fn to_vpn_ceil(&self) -> VirtPageNum {
        if self.0 == 0 {
            1.into()
        } else {
            VirtPageNum(((self.0 - 1) >> PAGE_OFFSET) + 1)
        }
    }
}

impl From<PhysAddr> for PhysPageNum {
    fn from(pa: PhysAddr) -> Self {
        Self(pa.0 >> PAGE_OFFSET)
    }
}

impl From<PhysPageNum> for PhysAddr {
    fn from(ppn: PhysPageNum) -> Self {
        Self(ppn.0 << PAGE_OFFSET)
    }
}

impl From<VirtAddr> for VirtPageNum {
    fn from(va: VirtAddr) -> Self {
        Self(va.0 >> PAGE_OFFSET)
    }
}

impl From<VirtPageNum> for VirtAddr {
    fn from(vpn: VirtPageNum) -> Self {
        Self(vpn.0 << PAGE_OFFSET)
    }
}


impl ops::Add<usize> for VirtPageNum {
    type Output = VirtPageNum;
    fn add(self, rhs: usize) -> VirtPageNum {
        return VirtPageNum(self.0 + rhs);
    }
}

impl ops::AddAssign<usize> for VirtPageNum {
    fn add_assign(&mut self, rhs: usize) { 
        self.0 += rhs;
    }
}

impl ops::Sub<usize> for VirtPageNum {
    type Output = VirtPageNum;
    fn sub(self, rhs: usize) -> VirtPageNum {
        return VirtPageNum(self.0 - rhs);
    }
}

impl ops::Sub<VirtPageNum> for VirtPageNum {
    type Output = usize;
    fn sub(self, rhs: VirtPageNum) -> usize {
        return self.0 - rhs.0;
    }
}

impl ops::SubAssign<usize> for VirtPageNum {
    fn sub_assign(&mut self, rhs: usize) { 
        self.0 -= rhs;
    }
}

impl ops::Add<VirtPageNum> for usize {
    type Output = VirtPageNum;
    fn add(self, rhs: VirtPageNum) -> VirtPageNum {
        return rhs + self;
    }
}

impl ops::Add<usize> for PhysPageNum {
    type Output = PhysPageNum;
    fn add(self, rhs: usize) -> PhysPageNum {
        return PhysPageNum(self.0 + rhs);
    }
}

impl ops::AddAssign<usize> for PhysPageNum {
    fn add_assign(&mut self, rhs: usize) { 
        self.0 += rhs;
    }
}

impl ops::Sub<usize> for PhysPageNum {
    type Output = PhysPageNum;
    fn sub(self, rhs: usize) -> PhysPageNum {
        return PhysPageNum(self.0 - rhs);
    }
}

impl ops::Sub<PhysPageNum> for PhysPageNum {
    type Output = usize;
    fn sub(self, rhs: PhysPageNum) -> usize {
        return self.0 - rhs.0;
    }
}

impl ops::SubAssign<usize> for PhysPageNum {
    fn sub_assign(&mut self, rhs: usize) { 
        self.0 -= rhs;
    }
}

impl ops::Add<PhysPageNum> for usize {
    type Output = PhysPageNum;
    fn add(self, rhs: PhysPageNum) -> PhysPageNum {
        return rhs + self;
    }
}

impl VirtPageNum {
    /// Get the L2/L1/L0 bits from the virtual page number
    /// # Description
    /// Get the L2/L1/L0 bits from the virtual page number, which looks something like this:  
    /// ` 63                           3938       3029       2120       12 11           0`  
    /// ` |            EXT              ||   L2    ||   L1    ||    L0   | |   offset   |`  
    /// `[XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX XXXX`  
    /// # Example
    /// ```
    /// let vpn: VirtPageNum = va.into();
    /// let (l2, l1, l0) = vpn.indexes();
    /// ```
    /// # Return
    /// Returns the three indexes of the virtual page number
    pub fn indexes(&self) -> [usize; 3] {
        return [
            (self.0 >> 18) & 0b1_1111_1111,
            (self.0 >>  9) & 0b1_1111_1111,
            (self.0 >>  0) & 0b1_1111_1111,
        ];
    }
}

impl StepUp for VirtAddr {
    fn step_up(&mut self) {
        self.0 += 1;
    }
}

impl StepDown for VirtAddr {
    fn step_down(&mut self) {
        self.0 -= 1;
    }
}

impl StepUp for PhysAddr {
    fn step_up(&mut self) {
        self.0 += 1;
    }
}

impl StepDown for PhysAddr {
    fn step_down(&mut self) {
        self.0 -= 1;
    }
}

impl StepUp for VirtPageNum {
    fn step_up(&mut self) {
        self.0 += 1;
    }
}

impl StepDown for VirtPageNum {
    fn step_down(&mut self) {
        self.0 -= 1;
    }
}

impl StepUp for PhysPageNum {
    fn step_up(&mut self) {
        self.0 += 1;
    }
}

impl StepDown for PhysPageNum {
    fn step_down(&mut self) {
        self.0 -= 1;
    }
}

pub type VARange = Range<VirtAddr>;
pub type PARange = Range<PhysAddr>;
pub type VPNRange = Range<VirtPageNum>;
pub type PPNRange = Range<PhysPageNum>;

impl PhysPageNum {
    // FIXME: very slow
    pub unsafe fn clear_content(&self) {
        for i in 0..PAGE_SIZE {
            (PhysAddr::from(*self) + i).write_volatile(&0u8);
        }
    }
}

impl VirtPageNum {
    // FIXME: very slow
    pub unsafe fn clear_content(&self) {
        for i in 0..PAGE_SIZE {
            (VirtAddr::from(*self) + i).write_volatile(&0u8);
        }
    }
}