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

pub struct Arc<'a, T> {
    ptr: NonNull<ArcInner<T>>,
    marker: PhantomData<ArcInner<&'a T>>
}

pub struct ArcInner<T> {
    rc: AtomicUsize,
    data: T
}

impl<'a, T> Arc<'a, T> {
    pub fn new_in(bump: &'a Bump, data: T) -> Arc<'a, T> {
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
}

impl<'a, T> Deref for Arc<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        let inner = unsafe { self.ptr.as_ref() };
        return &inner.data;
    }
}

impl<'a, T> Clone for Arc<'a, T> {
    fn clone(&self) -> Arc<'a, T> {
        let inner = unsafe { self.ptr.as_ref() };

        let old_rc = inner.rc.fetch_add(1, Ordering::Relaxed);

        assert!(old_rc <= usize::MAX);

        return Self {
            ptr: self.ptr,
            marker: PhantomData
        };
    }
}

impl<'a, T> Drop for Arc<'a, T> {
    fn drop(&mut self) {
        let inner = unsafe { self.ptr.as_ref() };

        if inner.rc.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        core::sync::atomic::fence(Ordering::Acquire);

        unsafe { Box::from_raw(self.ptr.as_ptr()) };
    }
}

unsafe impl<'a, T: Sync + Send> Send for Arc<'a, T> {}
unsafe impl<'a, T: Sync + Send> Sync for Arc<'a, T> {}