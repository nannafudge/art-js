extern crate alloc;

use wasm_bindgen::prelude::*;

use lock_api::{
    RawMutex,
    GuardNoSend
};

use futures_intrusive::sync::{
    GenericMutex,
    GenericMutexGuard,
    GenericMutexLockFuture
};

use js_sys::{
    SharedArrayBuffer,
    Uint8Array,
    Atomics
};

use core::{
    cell::RefCell,
    default::Default,
    marker::PhantomData,
    ptr::NonNull,
    sync::atomic::AtomicUsize,
    sync::atomic::Ordering,
    ops::Deref
};

use bumpalo::{
    Bump,
    boxed::Box
};

use crate::log::*;

const IS_LOCKED: usize = 1 << 0;
const IS_LOCKED_JS: i32 = 1 << 0;

#[wasm_bindgen]
pub struct AtomicLock(AtomicUsize);

#[wasm_bindgen]
pub struct AtomicLockJS {
    view: Option<RefCell<Uint8Array>>
}

impl Default for AtomicLock {
    fn default() -> Self {
        return AtomicLock(AtomicUsize::new(0));
    }
}

unsafe impl RawMutex for AtomicLock {
    const INIT: AtomicLock = AtomicLock(AtomicUsize::new(0));

    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        self.try_lock();
    }

    fn try_lock(&self) -> bool {
        return self.0.fetch_or(IS_LOCKED, Ordering::Acquire) == 0;
    }

    unsafe fn unlock(&self) {
        self.0.fetch_and(!IS_LOCKED, Ordering::AcqRel);
    }
}

unsafe impl RawMutex for AtomicLockJS {
    const INIT: AtomicLockJS = AtomicLockJS { view: None };

    type GuardMarker = GuardNoSend;

    fn lock(&self) {
        self.try_lock();
    }

    fn try_lock(&self) -> bool {
        return Atomics::or(self.view.as_ref().expect("AtomicLockJS is uninitialized").borrow().deref(), 0, IS_LOCKED_JS) == Ok(0);
    }

    unsafe fn unlock(&self) {
        Atomics::and(self.view.as_ref().expect("AtomicLockJS is uninitialized").borrow().deref(), 0, !IS_LOCKED_JS);
    }
}

impl AtomicLock {
    pub fn new() -> Self {
        return Self::default();
    }

    pub fn is_locked(&self) -> bool {
        return self.0.load(Ordering::Relaxed) == IS_LOCKED;
    }
}

// TODO: Make this return a Result::Err, if the Uint8Array size < 1
impl AtomicLockJS {
    pub fn new(view: RefCell<Uint8Array>) -> Self {
        return Self {
            view: Some(view)
        };
    }

    pub fn is_locked(&self) -> bool {
        return Atomics::load(self.view.as_ref().expect("AtomicLockJS is uninitialized").borrow().deref(), 0) == Ok(IS_LOCKED_JS);
    }
}

#[derive(Debug)]
pub struct Arc<T> {
    ptr: NonNull<ArcInner<T>>,
    marker: PhantomData<ArcInner<T>>
}

#[derive(Debug)]
pub struct ArcInner<T> {
    rc: AtomicUsize,
    data: T
}

impl<T> Arc<T> {
    pub fn new_in(bump: &Bump, data: T) -> Arc<T> {
        let boxed: Box<ArcInner<T>> = Box::new_in(
            ArcInner{
                rc: AtomicUsize::new(1),
                data
            },
            bump
        );

        return Self {
            ptr: NonNull::new(Box::into_raw(boxed)).unwrap(),
            marker: PhantomData
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.is_unique() {
            return unsafe { Some(self.get_mut_unchecked()) };
        }

        return None;
    }

    pub unsafe fn get_mut_unchecked(&mut self) -> &mut T {
        return &mut (*self.ptr.as_ptr()).data;
    }

    pub fn is_unique(&mut self) -> bool {
        let inner = unsafe { self.ptr.as_ref() };
        return inner.rc.load(Ordering::Acquire) == 1;
    }

    pub fn ref_count(&self) -> usize {
        let inner = unsafe { self.ptr.as_ref() };
        return inner.rc.load(Ordering::SeqCst);
    }
}

impl<T> Deref for Arc<T> {
    type Target = T;

    fn deref(&self) -> &T {
        let inner = unsafe { self.ptr.as_ref() };
        return &inner.data;
    }
}

impl<T> Clone for Arc<T> {
    fn clone(&self) -> Arc<T> {
        let inner = unsafe { self.ptr.as_ref() };

        let old_rc = inner.rc.fetch_add(1, Ordering::Relaxed);

        assert!(old_rc <= usize::MAX);

        return Self {
            ptr: self.ptr,
            marker: PhantomData
        };
    }
}

impl<T> Drop for Arc<T> {
    fn drop(&mut self) {
        let inner = unsafe { self.ptr.as_ref() };

        if inner.rc.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        core::sync::atomic::fence(Ordering::Acquire);

        unsafe { Box::from_raw(self.ptr.as_ptr()) };
    }
}

unsafe impl<'a, T: Sync + Send> Send for Arc<T> {}
unsafe impl<'a, T: Sync + Send> Sync for Arc<T> {}