//The MIT License (MIT)
//
//Copyright (c) 2014 Mathijs van de Nes
//
//Permission is hereby granted, free of charge, to any person obtaining a copy
//of this software and associated documentation files (the "Software"), to deal
//in the Software without restriction, including without limitation the rights
//to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//copies of the Software, and to permit persons to whom the Software is
//furnished to do so, subject to the following conditions:
//
//The above copyright notice and this permission notice shall be included in all
//copies or substantial portions of the Software.
//
//THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//SOFTWARE.

//! A RISC-V Mutex

use core::cell::UnsafeCell;
use core::marker::Sync;
use core::ops::{Drop, Deref, DerefMut};
use core::fmt;
use core::option::Option::{self, None, Some};
use core::default::Default;
use crate::{arch, panic, process::IntrLockGuard};
use crate::process::my_cpu;
use crate::panic_println;
use crate::arch::hart_id;

// TODO: move amoswap.w.aq into Rust
extern "C" {
    /// `spin_acquire` defined in `spinlock.c`
    fn spin_acquire(locked: &u32);
    /// `spin_release` defined in `spinlock.c`
    fn spin_release(locked: &u32);
}

/// A RISC-V Mutex.
pub struct Mutex<T: ?Sized> {
    /// Indicate whether data is locked. Will be passed into C code.
    lock: u32,
    /// Lock name, for debug use
    name: &'static str,
    /// HartID holding the lock
    hart: UnsafeCell<i64>,
    /// Save actual data
    data: UnsafeCell<T>,
}

/// A guard to which the protected data can be accessed
///
/// When the guard falls out of scope it will release the lock.
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a u32,
    mutex: &'a Mutex<T>,
    data: &'a mut T,
    intr_lock: IntrLockGuard<'a>,
}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(user_data: T, name: &'static str) -> Mutex<T> {
        Mutex {
            lock: 0,
            data: UnsafeCell::new(user_data),
            hart: UnsafeCell::new(-1),
            name,
        }
    }

    /// Consumes this mutex, returning the underlying data.
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        let Mutex { data, .. } = self;
        data.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    fn obtain_lock(&self) {
        unsafe { spin_acquire(&self.lock); }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        let intr_lock = my_cpu().intr_lock.lock();
        // panic_println!("{} lock on {}", self.name, arch::hart_id());
        if unsafe { self.holding() } {
            panic!("lock {}: hart {} already holding the lock!", self.name, arch::hart_id());
        }
        self.obtain_lock();
        if self.lock != 1 {
            panic!("lock {}: not locked!", self.name);
        }
        unsafe { *self.hart.get() = arch::hart_id() as i64; }
        MutexGuard {
            lock: &self.lock,
            mutex: self,
            data: unsafe { &mut *self.data.get() },
            intr_lock
        }
    }

    unsafe fn holding(&self) -> bool {
        let intr_lock = my_cpu().intr_lock.lock();
        if self.lock == 1 && *self.hart.get() == arch::hart_id() as i64 {
            return true;
        }
        false
    }
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref<'b>(&'b self) -> &'b T { &*self.data }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    fn deref_mut<'b>(&'b mut self) -> &'b mut T { &mut *self.data }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    /// The dropping of the MutexGuard will release the lock it was created from.
    fn drop(&mut self) {
        if unsafe { !self.mutex.holding() } {
            panic!("lock {}: unlock from another hart {}!", self.mutex.name, arch::hart_id());
        }
        unsafe { *self.mutex.hart.get() = -1; }
        unsafe { spin_release(&self.lock); }
        // panic_println!("{} unlock on {}", self.mutex.name, arch::hart_id());
    }
}
