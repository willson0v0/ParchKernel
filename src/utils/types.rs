
use core::fmt::{self, Debug, Formatter};
use core::ops;
use core::ptr::{read_volatile, write_volatile};

#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct VirtAddr(pub usize);


#[repr(C)]
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct PhysAddr(pub usize);

impl From<usize> for PhysAddr       { fn from(num: usize) -> Self { Self(num) } }
impl From<usize> for VirtAddr       { fn from(num: usize) -> Self { Self(num) } }

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
}