use core::cell::{RefCell, UnsafeCell};

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{Ordering, AtomicBool};
use core::option::Option;
use alloc::string::String;
use crate::interrupt::{get_hart_id, pop_intr_off, push_intr_off};

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

pub struct SpinMutex<T> {
    is_acquired  : AtomicBool,
    name        : String,
    data        : UnsafeCell<T>,
    acquired_by : RefCell<Option<usize>>,
    did_push_off : RefCell<bool>
}

impl<T> SpinMutex<T> {
    pub fn new(name: &str, data: T) -> Self {
        Self {
            is_acquired: AtomicBool::new(false),
            acquired_by: RefCell::new(None),
            name: String::from(name),
            data: UnsafeCell::new(data),
            did_push_off: RefCell::new(true)
        }
    }
    
    pub fn acquire_no_off(&self) -> MutexGuard<'_, T> {
        if self.is_acquired.load(Ordering::Relaxed) && self.acquired_by.borrow().unwrap() == get_hart_id() {
            panic!("Acquiring acquired lock")
        }
        while !self.is_acquired.swap(true, Ordering::AcqRel) {
            // spin wait
        }
        // change after lock has been successfully acquired, thus refcell is safe to change
        *self.acquired_by.borrow_mut() = Some(get_hart_id());
        *self.did_push_off.borrow_mut() = false;
        MutexGuard{mutex: self}
    }
}

impl<T> Mutex<T> for SpinMutex<T> {
    fn acquire(&self) -> MutexGuard<'_, T> {
        push_intr_off();
        if self.is_acquired.load(Ordering::Relaxed) && self.acquired_by.borrow().unwrap() == get_hart_id() {
            panic!("Acquiring acquired lock")
        }
        while !self.is_acquired.swap(true, Ordering::AcqRel) {
            // spin wait
        }
        // change after lock has been successfully acquired, thus refcell is safe to change
        *self.acquired_by.borrow_mut() = Some(get_hart_id());
        *self.did_push_off.borrow_mut() = true;
        MutexGuard{mutex: self}
    }

    fn release(&self) {
        self.is_acquired.store(false, Ordering::Release);
        if *self.did_push_off.borrow() {
            pop_intr_off();
        }
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
        // TODO: Check if is a Kernel thread for Proc is acquiring SleepMutex. Scheduler kernel thread is not allowed to use this.
        while !self.is_acquired.swap(true, Ordering::AcqRel) {
            // TODO: Yield cpu
        }
        // change after lock has been successfully acquired, thus refcell is safe to change
        *self.acquired_by.borrow_mut() = Some(get_hart_id());
        MutexGuard{mutex: self}
    }

    fn release(&self) {
        self.is_acquired.store(false, Ordering::Release)
        // TODO: notify yielded Process
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