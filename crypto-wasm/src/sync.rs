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

use core::{
    sync::atomic::{
        AtomicUsize,
        Ordering
    }
};

use core::default::Default;

use crate::log::*;

/*use alloc::{
    vec::Vec,
    boxed::Box,
    sync::Arc
};

use core::{
    usize,
    mem,
    cmp::{
        Ord,
        Ordering,
        PartialOrd,
        Eq,
        PartialEq
    },
    iter::Iterator,
    default::Default,
    borrow::Borrow,
    marker::{
        Sync,
        PhantomData
    },
    sync::atomic::{
        AtomicUsize
    },
    task::{
        Poll,
        Context,
        Waker
    },
    future::{
        Future,
    },
    pin::Pin,
    ops::Deref
};*/

use async_trait::async_trait;

const IS_LOCKED: usize = 1 << 0;

#[wasm_bindgen]
pub struct AtomicLock(AtomicUsize);

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

impl Default for AtomicLock {
    fn default() -> Self {
        return AtomicLock(AtomicUsize::new(0));
    }
}

impl AtomicLock {
    fn is_locked(&self) -> bool {
        return self.0.load(Ordering::Relaxed) == 0;
    }
}
/*const IS_LOCKED: usize = 1 << 0;
const HAS_WAITERS: usize = 1 << 1;

enum Waiter {
    Waiting(Waker),
    Woken,
}

impl Waiter {
    fn register(&mut self, waker: &Waker) {
        match self {
            Self::Waiting(w) if waker.will_wake(w) => {}
            _ => *self = Self::Waiting(waker.clone())
        }
    }

    fn wake(&mut self) {
        match mem::replace(self, Self::Woken) {
            Self::Waiting(waker) => waker.wake(),
            Self::Woken => {}
        }
    }
}

pub struct Mutex<T: ?Sized> {
    state: AtomicUsize,
    data: Arc<T>,
    waiter: Arc<Waiter>
}

pub struct MutexGuard<'a, T: ?Sized> {
    mutex: Mutex<T>,
    marker: PhantomData<&'a T>,
}

pub struct MutexLockFuture<'a, T: ?Sized> {
    mutex: Mutex<T>,
    marker: PhantomData<&'a T>
}

impl<'a, T: ?Sized> Mutex<T> {
    pub fn lock(self: &Self) -> MutexLockFuture<'a, T> {
        return MutexLockFuture::<'a> {
            mutex: *self,
            marker: PhantomData
        }
    }
}

impl<'a, T: ?Sized> Deref for MutexLockFuture<'a, T> {
    type Target = Mutex<T>;

    fn deref(&self) -> &Self::Target {
        return &self.mutex;
    }
}

impl<'a, T> Future for MutexLockFuture<'a, T> {
    type Output = MutexGuard<'a, T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut data = self.mutex.data;

        if let Some(mut_lock) = Arc::get_mut(&mut data) {

        }

        return Poll::Pending;
    }
}*/