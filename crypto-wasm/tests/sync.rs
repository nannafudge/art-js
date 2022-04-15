#![cfg(test)]
extern crate alloc;
#[macro_use]
extern crate crypto_art;

use wasm_bindgen_test::*;
use crypto_art::{
    log::*,
    sync::AtomicLock,
    sync::AtomicLockJS
};

use lock_api::{
    Mutex,
    RawMutex,
    MutexGuard
};

use js_sys::{
    SharedArrayBuffer,
    Uint8Array
};

use alloc::sync::Arc;
use core::cell::RefCell;

// TODO: Test this with Web Workers once I've begun work implementing multithreading via such
#[wasm_bindgen_test]
fn test_atomic_lock_access() {
    let mutex: Arc<AtomicLock> = Arc::new(AtomicLock::default());
    let l1: Arc<AtomicLock> = mutex.clone();
    let l2: Arc<AtomicLock> = mutex.clone();

    // Lock 1 lock
    assert!(l1.try_lock());
    // Lock 2 reference attempt to lock
    assert!(!l2.try_lock());

    // Lock 1 unlock, unsafe block handled by lock_api Mutex.
    // I am sure as well that lock_api handles cases in which other references can force 'unlock' the lock
    unsafe {
        l1.unlock();
        assert!(!l2.is_locked());
    }

    assert!(l2.try_lock());
}

// Test atomic operation/locking. To be used with a FIFO buffer/circular buffer for cross-memory access.
// Test RAII dropping of Mutex guards as well
#[wasm_bindgen_test]
fn test_mutex_atomic_lock() {
    let mutex: Mutex<AtomicLock, usize> = Mutex::new(0);

    {
        let mut guard: MutexGuard<AtomicLock, usize> = mutex.try_lock().expect("Error acquiring Mutex guard");
        *guard += 1;
        
        assert!(mutex.try_lock().is_none());
    }

    {
        let mut guard: MutexGuard<AtomicLock, usize> = mutex.try_lock().expect("Error acquiring Mutex guard");
        *guard += 1;
        
        assert!(mutex.try_lock().is_none());
        assert!(mutex.try_lock().is_none());
    }

    assert!(mutex.into_inner() == 2);
}

// TODO: Test this with Web Workers once I've begun work implementing multithreading via such
#[wasm_bindgen_test]
fn test_js_atomic_lock_access() {
    let sab: SharedArrayBuffer = SharedArrayBuffer::new(8);
    let lock: Arc<AtomicLockJS> = Arc::new(
        AtomicLockJS::new(
            RefCell::new(Uint8Array::new(&sab))
        )
    );

    let l1: Arc<AtomicLockJS> = lock.clone();
    let l2: Arc<AtomicLockJS> = lock.clone();

    // Lock 1 lock
    assert!(l1.try_lock());
    // Lock 2 reference attempt to lock
    assert!(!l2.try_lock());

    // Lock 1 unlock, unsafe block handled by lock_api Mutex.
    // I am sure as well that lock_api handles cases in which other references can force 'unlock' the lock
    unsafe {
        l1.unlock();
        assert!(!l2.is_locked());
    }

    assert!(l2.try_lock());
}

// Test atomic operation/locking. To be used with a FIFO buffer/circular buffer for cross-memory access.
// Test RAII dropping of Mutex guards as well
#[wasm_bindgen_test]
fn test_mutex_js_atomic_lock() {
    let sab: SharedArrayBuffer = SharedArrayBuffer::new(8);
    let lock: AtomicLockJS = AtomicLockJS::new(RefCell::new(Uint8Array::new(&sab)));
    let mutex: Mutex<AtomicLockJS, usize> = Mutex::const_new(lock, 0);

    {
        let mut guard: MutexGuard<AtomicLockJS, usize> = mutex.try_lock().expect("Error acquiring Mutex guard");
        *guard += 1;
        
        assert!(mutex.try_lock().is_none());
    }

    {
        let mut guard: MutexGuard<AtomicLockJS, usize> = mutex.try_lock().expect("Error acquiring Mutex guard");
        *guard += 1;
        
        assert!(mutex.try_lock().is_none());
        assert!(mutex.try_lock().is_none());
    }

    assert!(mutex.into_inner() == 2);
}