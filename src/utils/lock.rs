use core::cell::{RefCell, UnsafeCell};

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{Ordering, AtomicBool};
use core::option::Option;
use alloc::string::String;
use crate::process::{get_hart_id, pop_intr_off, push_intr_off, get_processor};

pub trait Mutex<T> {
    fn acquire(&self) -> MutexGuard<'_, T>;
    fn release(&self);
    fn get_data(&self) -> &mut T;
    fn get_name(&self) -> String;
    fn locked(&self) -> bool;
    unsafe fn force_relock(&self);
    unsafe fn force_unlock(&self);
    unsafe fn from_locked(&self) -> MutexGuard<'_, T>;
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

impl<T> MutexGuard<'_, T> {
    pub unsafe fn force_relock(&self) {
        self.mutex.force_relock();
    }

    pub unsafe fn force_unlock(&self) {
        self.mutex.force_unlock();
    }

    pub fn check_intergrity(&self) {
        if !self.mutex.locked() {
            panic!("MutexGuard compromised.");
        }
    }
}

// TODO: Implement R/W lock
pub struct SpinMutex<T> {
    is_acquired  : AtomicBool,
    name        : String,
    data        : UnsafeCell<T>,
    did_push_off : UnsafeCell<bool>
}

impl<T> SpinMutex<T> {
    pub fn new(name: &str, data: T) -> Self {
        Self {
            is_acquired: AtomicBool::new(false),
            name: String::from(name),
            data: UnsafeCell::new(data),
            did_push_off: UnsafeCell::new(true)
        }
    }
}

impl<T> Mutex<T> for SpinMutex<T> {
    fn acquire(&self) -> MutexGuard<'_, T> {
        push_intr_off();
        while self.is_acquired.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            // spin wait
        }
        // change after lock has been successfully acquired, thus refcell is safe to change
        unsafe{*self.did_push_off.get() = true;}
        MutexGuard{mutex: self}
    }

    fn release(&self) {
        unsafe {self.force_unlock();}
        if unsafe{*self.did_push_off.get()} {
            pop_intr_off();
        }
    }

    fn get_data(&self) -> &mut T {
        unsafe {&mut *self.data.get()}
    }

    fn get_name(&self) -> String{
        self.name.clone()
    }

    fn locked(&self) -> bool {
        self.is_acquired.load(Ordering::Relaxed)
    }

    unsafe fn force_relock(&self) {
        if self.is_acquired.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            panic!("Mutex must be unlocked to be force relock")
        }
    }

    unsafe fn force_unlock(&self) {
        if self.is_acquired.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            panic!("Mutex must be locked to be force unlock")
        }
    }

    unsafe fn from_locked(&self) -> MutexGuard<'_, T> {
        let result = MutexGuard{mutex: self};
        result.check_intergrity();
        result
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
        // TODO: Check if is Scheduler Kernel thread for Proc is acquiring SleepMutex. Scheduler kernel thread is not allowed to use this.
        while !self.is_acquired.swap(true, Ordering::SeqCst) {
            get_processor().suspend_switch();
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

    fn locked(&self) -> bool {
        self.is_acquired.load(Ordering::Relaxed)
    }

    unsafe fn force_relock(&self) {
        if self.is_acquired.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            panic!("Mutex must be unlocked to be force relock")
        }
    }

    unsafe fn force_unlock(&self) {
        if self.is_acquired.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            panic!("Mutex must be locked to be force unlock")
        }
    }

    unsafe fn from_locked(&self) -> MutexGuard<'_, T> {
        let result = MutexGuard{mutex: self};
        result.check_intergrity();
        result
    }
}

unsafe impl<T> Send for SpinMutex<T> where T: Send {}
unsafe impl<T> Sync for SpinMutex<T> where T: Send {}
unsafe impl<T> Send for MutexGuard<'_, T> where T: Send {}
unsafe impl<T> Sync for MutexGuard<'_, T> where T: Send + Sync {}