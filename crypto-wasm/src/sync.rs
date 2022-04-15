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
    sync::atomic::{
        AtomicUsize,
        Ordering
    },
    default::Default,
    cell::RefCell,
    ops::Deref
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