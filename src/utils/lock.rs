use core::borrow::BorrowMut;
use core::cell::{RefCell, UnsafeCell};
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{Ordering, AtomicBool};
use core::option::Option;
use alloc::string::String;
use crate::interrupt::get_hart_id;

pub trait Mutex<T> {
    fn acquire(&self) -> MutexGuard<'_, T>;
    fn release(&self);
    fn get_data(&self) -> &mut T;
    fn get_name(&self) -> String;
}

pub struct MutexGuard<'a, T> {
    mutex: &'a dyn Mutex<T>
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.mutex.get_data()
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mutex.get_data()
    }
}

impl<T> Drop for MutexGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.release()
    }
}

#[derive(Debug)]
pub struct SpinMutex<T> {
    is_acquired  : AtomicBool,
    name        : String,
    data        : UnsafeCell<T>,
    acquired_by : RefCell<Option<usize>>
}

impl<T> SpinMutex<T> {
    pub fn new(name: String, data: T) -> Self {
        Self {
            is_acquired: AtomicBool::new(false),
            acquired_by: RefCell::new(None),
            name,
            data: UnsafeCell::new(data)
        }
    }
}

impl<T> Mutex<T> for SpinMutex<T> {
    fn acquire(&self) -> MutexGuard<'_, T> {
        while !self.is_acquired.swap(true, Ordering::AcqRel) {
            // spin wait
        }
        // change after lock has been successfully acquired, thus refcell is safe to change
        *self.acquired_by.borrow_mut() = Some(get_hart_id());
        MutexGuard{mutex: self}
    }

    fn release(&self) {
        self.is_acquired.store(false, Ordering::Release)
    }

    fn get_data(&self) -> &mut T {
        unsafe {&mut *self.data.get()}
    }

    fn get_name(&self) -> String{
        self.name.clone()
    }
}

pub struct SleepMutex<T> {
    is_acquired : AtomicBool,
    name        : String,
    data        : UnsafeCell<T>,
    acquired_by : RefCell<Option<usize>>
}


impl<T> SleepMutex<T> {
    pub fn new(name: String, data: T) -> Self {
        Self {
            is_acquired: AtomicBool::new(false),
            acquired_by: RefCell::new(None),
            name,
            data: UnsafeCell::new(data)
        }
    }
}

impl<T> Mutex<T> for SleepMutex<T> {
    fn acquire(&self) -> MutexGuard<'_, T> {
        while !self.is_acquired.swap(true, Ordering::AcqRel) {
            // TODO: Yield cpu
        }
        // change after lock has been successfully acquired, thus refcell is safe to change
        *self.acquired_by.borrow_mut() = Some(get_hart_id());
        MutexGuard{mutex: self}
    }

    fn release(&self) {
        self.is_acquired.store(false, Ordering::Release)
        // TODO: notify yielded CPU
    }

    fn get_data(&self) -> &mut T {
        unsafe {&mut *self.data.get()}
    }

    fn get_name(&self) -> String{
        self.name.clone()
    }
}

unsafe impl<T> Send for SpinMutex<T> where T: Send {}
unsafe impl<T> Sync for SpinMutex<T> where T: Send {}
unsafe impl<T> Send for MutexGuard<'_, T> where T: Send {}
unsafe impl<T> Sync for MutexGuard<'_, T> where T: Send + Sync {}