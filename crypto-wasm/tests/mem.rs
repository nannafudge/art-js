#![cfg(test)]
#[macro_use]
extern crate crypto_art;

use wasm_bindgen_test::*;

use core::cell::RefCell;

use ringbuffer::{
    RingBuffer,
    RingBufferExt,
    RingBufferRead,
    RingBufferWrite
};

use crypto_art::{
    mem::DataView,
    mem::SharedRingBuffer,
    sync::AtomicLockJS,
    log::*
};

use lock_api::{
    Mutex,
    RawMutex,
    MutexGuard
};

use serde::{Deserialize, Serialize};

use js_sys::{
    SharedArrayBuffer,
    Uint8Array
};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestObject {
    value: u32
}

fn is_zeroed(arr: &Uint8Array) -> bool {
    for i in 0..arr.length() {
        if arr.get_index(i) != 0 {
            return false;
        }
    }

    return true;
}

#[wasm_bindgen_test]
fn test_shared_data_view() {
    let shared: SharedArrayBuffer = SharedArrayBuffer::new(16);
    let view: DataView = DataView::new(&shared, 0, 2);

    view.set_uint8(0, 1);
    view.set_uint8(1, 255);

    assert_eq!(view.get_uint8(0), 1);
    assert_eq!(view.get_uint8(1), 255);
}

#[wasm_bindgen_test]
fn test_ring_buffer_enqueue_dequeue_sequential() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);
    srb.push(TestObject{ value: 32 });
    srb.push(TestObject{ value: 16 });
    srb.push(TestObject{ value: 0 });

    assert_eq!(srb.len(), 3);
    assert_eq!(srb.dequeue().expect("Could not dequeue object #1").value, 32);
    assert_eq!(srb.len(), 2);
    assert_eq!(srb.dequeue().expect("Could not dequeue object #2").value, 16);
    assert_eq!(srb.len(), 1);
    assert_eq!(srb.dequeue().expect("Could not dequeue object #3").value, 0);
    assert_eq!(srb.len(), 0);
}

#[wasm_bindgen_test]
fn test_ring_buffer_enqueue_dequeue_staggered() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);
    srb.push(TestObject{ value: 32 });
    srb.push(TestObject{ value: 16 });

    assert_eq!(srb.len(), 2);
    // TestObject{ value: 32 }
    assert_eq!(srb.dequeue().expect("Could not dequeue object #1").value, 32);
    assert_eq!(srb.len(), 1);

    srb.push(TestObject{ value: 0 });
    // TestObject{ value: 16 }
    assert_eq!(srb.dequeue().expect("Could not dequeue object #2").value, 16);
    assert_eq!(srb.len(), 1);
    // TestObject{ value: 0 }
    assert_eq!(srb.dequeue().expect("Could not dequeue object #3").value, 0);
    assert_eq!(srb.len(), 0);
}

#[wasm_bindgen_test]
fn test_ring_buffer_loop_fast() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);

    for _ in 0..4 {
        assert_eq!(srb.len(), 0);

        srb.push(TestObject{ value: 1 });
        srb.push(TestObject{ value: 2 });
        srb.push(TestObject{ value: 3 });
        srb.push(TestObject{ value: 4 });

        assert_eq!(srb.len(), 4);

        srb.dequeue();
        srb.dequeue();
        srb.dequeue();
        srb.dequeue();
    }
}

#[wasm_bindgen_test]
fn test_ring_buffer_mutex() {
    let srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);
    let lock: AtomicLockJS = AtomicLockJS::new(RefCell::new(srb.uint8_view()));
    let mutex: Mutex<AtomicLockJS, SharedRingBuffer<TestObject>> = Mutex::const_new(lock, srb);

    {
        let mut guard: MutexGuard<AtomicLockJS, SharedRingBuffer<TestObject>> = mutex.try_lock().expect("Error acquiring Mutex guard in block 1");

        guard.push(TestObject{ value: 32 });
        guard.push(TestObject{ value: 16 });
        guard.push(TestObject{ value: 0 });
    }

    {
        let mut guard: MutexGuard<AtomicLockJS, SharedRingBuffer<TestObject>> = mutex.try_lock().expect("Error acquiring Mutex guard in block 2");

        assert_eq!(guard.len(), 3);
        assert_eq!(guard.dequeue().expect("Could not dequeue object #1").value, 32);
    }

    {
        let mut guard: MutexGuard<AtomicLockJS, SharedRingBuffer<TestObject>> = mutex.try_lock().expect("Error acquiring Mutex guard in block 3");

        assert_eq!(guard.len(), 2);
        assert_eq!(guard.dequeue().expect("Could not dequeue object #2").value, 16);
        assert_eq!(guard.dequeue().expect("Could not dequeue object #2").value, 0);
    }

    assert_eq!(mutex.lock().len(), 0);
}

#[wasm_bindgen_test]
fn test_ring_buffer_mutex_loop_fast() {
    let srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);
    let lock: AtomicLockJS = AtomicLockJS::new(RefCell::new(srb.uint8_view()));
    let mutex: Mutex<AtomicLockJS, SharedRingBuffer<TestObject>> = Mutex::const_new(lock, srb);

    for _ in 0..4 {
        if let Some(mut guard) = mutex.try_lock() {
            assert_eq!(guard.len(), 0);
            guard.push(TestObject{ value: 1 });
            guard.push(TestObject{ value: 2 });
            guard.push(TestObject{ value: 3 });
            guard.push(TestObject{ value: 4 });
        }

        if let Some(mut guard) = mutex.try_lock() {
            assert_eq!(guard.len(), 4);
            assert_eq!(guard.dequeue().expect("Could not dequeue object #1").value, 1);
            assert_eq!(guard.dequeue().expect("Could not dequeue object #2").value, 2);
            assert_eq!(guard.dequeue().expect("Could not dequeue object #3").value, 3);
            assert_eq!(guard.dequeue().expect("Could not dequeue object #4").value, 4);
        }
    }

    assert_eq!(mutex.lock().len(), 0);
}