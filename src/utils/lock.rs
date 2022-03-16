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
    unsafe fn leak(&self) -> &mut T;
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
pub struct SpinMutex<T> {
    is_acquired  : AtomicBool,
    name        : String,
    data        : UnsafeCell<T>
}

impl<T> SpinMutex<T> {
    pub fn new(name: &str, data: T) -> Self {
        Self {
            is_acquired: AtomicBool::new(false),
            name: String::from(name),
            data: UnsafeCell::new(data)
        }
    }
}

impl<T> Mutex<T> for SpinMutex<T> {
    fn acquire(&self) -> MutexGuard<'_, T> {
        push_intr_off();
        while self.is_acquired.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            // spin wait
        }
        MutexGuard{mutex: self}
    }

    fn release(&self) {
        unsafe {self.force_unlock();}
        pop_intr_off();
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

    unsafe fn leak(&self) -> &mut T {
        &mut *self.data.get()
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
    
    unsafe fn leak(&self) -> &mut T {
        &mut *self.data.get()
    }
}

unsafe impl<T> Send for SpinMutex<T> where T: Send {}
unsafe impl<T> Sync for SpinMutex<T> where T: Send {}
unsafe impl<T> Send for MutexGuard<'_, T> where T: Send {}
unsafe impl<T> Sync for MutexGuard<'_, T> where T: Send + Sync {}

pub trait RWLock<T> {
    fn acquire_r(&self) -> RWLockReadGuard<'_, T>;
    fn acquire_w(&self) -> RWLockWriteGuard<'_, T>;
    fn release_r(&self);
    fn release_w(&self);
    fn get_data(&self) -> &mut T;
}

pub struct RWLockReadGuard<'a, T> {
    mutex: &'a dyn RWLock<T>
}

pub struct RWLockWriteGuard<'a, T> {
    mutex: &'a dyn RWLock<T>
}
pub struct SpinRWLock<T> {
    write_mutex         : AtomicBool,
    reader_count        : SpinMutex<usize>,
    data                : UnsafeCell<T>
}

impl<T> SpinRWLock<T> {
    pub fn new(data: T) -> Self {
        Self {
            write_mutex: AtomicBool::new(false),
            reader_count: SpinMutex::new("rw lock mutex", 0),
            data: UnsafeCell::new(data),
        }
    }
}

impl<T> RWLock<T> for SpinRWLock<T> {
    fn acquire_r(&self) -> RWLockReadGuard<'_, T> {
        // lock the lock itself;
        let mut lock_guard = self.reader_count.acquire();

        *lock_guard += 1;

        if *lock_guard == 1 {
            push_intr_off();
            // data alter, wait for write to finish
            while self.write_mutex.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                // spin wait
            }
        }
        
        RWLockReadGuard { mutex: self }
    }

    fn acquire_w(&self) -> RWLockWriteGuard<'_, T> {
        push_intr_off();
        while self.write_mutex.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            // spin wait
        }
        RWLockWriteGuard{mutex: self}
    }

    fn release_r(&self) {
        // try to lock lock itself;
        let mut lock_guard = self.reader_count.acquire();

        *lock_guard -= 1;

        if *lock_guard == 0 {
            if self.write_mutex.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_err() {
                panic!("RWLocked must be locked to be unlocked")
            }
            pop_intr_off();
        }
    }

    fn release_w(&self) {
        if self.write_mutex.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            panic!("RWLocked must be locked to be unlocked")
        }
        pop_intr_off();
    }

    fn get_data(&self) -> &mut T {
        unsafe {&mut *self.data.get()}
    }
}


impl<T> Deref for RWLockReadGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.mutex.get_data()
    }
}

impl<T> Deref for RWLockWriteGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.mutex.get_data()
    }
}

impl<T> DerefMut for RWLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mutex.get_data()
    }
}

impl<T> Drop for RWLockReadGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.release_r()
    }
}

impl<T> Drop for RWLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.mutex.release_w()
    }
}


unsafe impl<T> Send for SpinRWLock<T> where T: Send {}
unsafe impl<T> Sync for SpinRWLock<T> where T: Send {}
unsafe impl<T> Send for RWLockReadGuard<'_, T> where T: Send {}
unsafe impl<T> Sync for RWLockReadGuard<'_, T> where T: Send + Sync {}
unsafe impl<T> Send for RWLockWriteGuard<'_, T> where T: Send {}
unsafe impl<T> Sync for RWLockWriteGuard<'_, T> where T: Send + Sync {}