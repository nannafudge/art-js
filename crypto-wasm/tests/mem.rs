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
    mem::AllocatorPool,
    mem::AllocatorCell,
    sync::AtomicLockJS,
    log::*
};

use lock_api::{
    Mutex,
    MutexGuard
};

use serde::{Deserialize, Serialize};

use js_sys::{
    SharedArrayBuffer
};

use bumpalo::{
    Bump,
    boxed::Box,
    collections::Vec
};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestObject {
    value: u32
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

#[wasm_bindgen_test]
fn test_ring_buffer_auxiliary_functions() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);
    assert_eq!(srb.get_head_index(), 0);
    srb.push(TestObject{ value: 32 });
    assert_eq!(srb.get_head_index(), 1);
    srb.push(TestObject{ value: 16 });
    assert_eq!(srb.get_head_index(), 2);
    srb.push(TestObject{ value: 0 });
    assert_eq!(srb.get_head_index(), 3);

    assert_eq!(32, srb.read(0).expect("No value present").value);
    assert_eq!(16, srb.read(1).expect("No value present").value);
    assert_eq!(0, srb.read(2).expect("No value present").value);

    assert_eq!(srb.get_tail_index(), 0);
    srb.dequeue();
    assert_eq!(srb.get_tail_index(), 1);
    srb.dequeue();
    assert_eq!(srb.get_tail_index(), 2);
    srb.dequeue();
    assert_eq!(srb.get_tail_index(), 3);

    assert!(srb.read(0).is_none());
    assert!(srb.read(1).is_none());
    assert!(srb.read(2).is_none());

    assert!(srb.read(12345).is_none());

    srb.write(&TestObject{ value: 12345 }, 0);
    assert_eq!(12345, srb.read(0).expect("No value present").value);
}

/* 
  I hate this functionality so much, why is it in the trait? Maybe I'll find a use
  for it eventually. Implemented as to the specification:

  Gets a value relative to the current index. 0 is the next index to be written to with push.
  -1 and down are the last elements pushed and 0 and up are the items that were pushed the longest ago.
*/
#[wasm_bindgen_test]
fn test_ring_buffer_parse_index() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);

    srb.push(TestObject{ value: 32 });
    assert_eq!(Some(0), srb.parse_index(-1));
    assert_eq!(None, srb.parse_index(-2));

    srb.fill_with(|| return TestObject{ value: 32 });

    // Head is at 0, relative to head @ 0, -1, aka last written index, is 3
    assert_eq!(Some(0), srb.parse_index(0));
    assert_eq!(Some(3), srb.parse_index(-1));
    assert_eq!(Some(2), srb.parse_index(-2));
    assert_eq!(Some(1), srb.parse_index(-3));
    assert_eq!(Some(0), srb.parse_index(-4));
    assert_eq!(None, srb.parse_index(-5));

    srb.skip();

    assert_eq!(Some(0), srb.parse_index(0));
    assert_eq!(Some(3), srb.parse_index(-1));
    assert_eq!(Some(2), srb.parse_index(-2));
    assert_eq!(Some(1), srb.parse_index(-3)); // Head has been erased here
    assert_eq!(None, srb.parse_index(-4));

    srb.push(TestObject{ value: 32 });

    assert_eq!(Some(1), srb.parse_index(0));
    assert_eq!(Some(0), srb.parse_index(-1));
    assert_eq!(Some(3), srb.parse_index(-2));
    assert_eq!(Some(2), srb.parse_index(-3));
    assert_eq!(Some(1), srb.parse_index(-4));
    assert_eq!(None, srb.parse_index(-5));
}

#[wasm_bindgen_test]
fn test_ring_buffer_get() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);

    srb.push(TestObject{ value: 32 });
    assert_eq!(32, srb.get(-1).expect("Unable to get value at -1").value);
    assert_eq!(32, srb.get_raw(-1).expect("Unable to get value at -1").value);
    assert_eq!(32, srb.get_absolute(0).expect("Unable to get value at 0").value);
    assert_eq!(32, srb.get_absolute_mut(0).expect("Unable to get value at 0").value);

    // Test erronous values
    assert!(srb.get(-2).is_none());
    assert!(srb.get_raw(-3).is_none());
    assert!(srb.get_absolute(3).is_none());
    assert!(srb.get_absolute_mut(6).is_none());

    assert_eq!(32, srb.dequeue().expect("Could not dequeue object").value);

    assert!(srb.get(-1).is_none());
    assert!(srb.get_raw(-1).is_none());
    assert!(srb.get_absolute(0).is_none());
    assert!(srb.get_absolute_mut(0).is_none());

    // retest erronous values
    assert!(srb.get(-2).is_none());
    assert!(srb.get_raw(-2).is_none());
    assert!(srb.get_absolute(3).is_none());
    assert!(srb.get_absolute_mut(6).is_none());
}

#[wasm_bindgen_test]
fn test_ring_buffer_fill_with() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);

    srb.fill_with(|| return TestObject{ value: 32 });

    assert_eq!(4, srb.len());

    assert_eq!(32, srb.dequeue().expect("Could not dequeue object").value);
    assert_eq!(32, srb.dequeue().expect("Could not dequeue object").value);
    assert_eq!(32, srb.dequeue().expect("Could not dequeue object").value);
    assert_eq!(32, srb.dequeue().expect("Could not dequeue object").value);

    assert_eq!(0, srb.len());
}

#[wasm_bindgen_test]
fn test_ring_buffer_clear() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);

    srb.fill_with(|| return TestObject{ value: 32 });

    assert_eq!(4, srb.len());

    srb.clear();

    assert_eq!(0, srb.len());

    assert!(srb.get(-1).is_none());
}

#[wasm_bindgen_test]
fn test_ring_buffer_skip() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);

    srb.fill_with(|| return TestObject{ value: 32 });

    assert_eq!(4, srb.len());

    srb.skip();

    assert_eq!(3, srb.len());
    srb.push(TestObject{ value: 16 });

    assert_eq!(16, srb.get(-1).expect("Could not get object at last index").value);
}

#[wasm_bindgen_test]
fn test_ring_buffer_subscript() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(4);

    srb.fill_with(|| return TestObject{ value: 32 });

    for i in 0..srb.capacity() - 1 {
        assert_eq!(32, srb[i as isize].value);
        assert_eq!(32, srb[!(i as isize)].value);
    }
}

/*
* TODO: Further testing of the allocator_pool methods
*/
#[wasm_bindgen_test]
fn test_allocator_pool() {
    let alloc: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);
    let mut pool: AllocatorPool = AllocatorPool::new(&alloc);

    for i in 0..3 {
        pool.initialize::<usize>(i, 4);
    }

    for i in 0..pool.len() - 1 {
        assert!(pool.has(i));
    }
}

#[wasm_bindgen_test]
fn test_allocator_pool_with_init() {
    let alloc: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);
    let pool: AllocatorPool = AllocatorPool::new_with_init::<usize>(&alloc, 4, 4);

    assert_eq!(pool.len(), 4);

    for i in 0..pool.len() - 1 {
        assert!(pool.has(i));
    }
}

#[wasm_bindgen_test]
fn test_allocator_pool_cell_drop() {
    let alloc: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);
    let mut pool: AllocatorPool = AllocatorPool::new_with_init::<usize>(&alloc, 4, 4);

    let previous_allocated_bytes: usize = pool.get(0).allocated_bytes();

    // After this block is complete, drop should be called on the AllocatorCell, which resets the allocator
    {
        let cell: &mut AllocatorCell = &mut pool.get_mut(0).clone();

        cell.alloc_slice_fill_with(64, |_| return 1);

        assert_ne!(previous_allocated_bytes, cell.allocated_bytes());
    }

    let cell: &mut AllocatorCell = &mut pool.get_mut(0).clone();

    // Memory should have been reset as the pool cell no longer had any references living to it
    assert_eq!(previous_allocated_bytes, cell.allocated_bytes());
}