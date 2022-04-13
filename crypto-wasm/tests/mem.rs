#![cfg(test)]
#[macro_use]
extern crate crypto_art;

use wasm_bindgen_test::*;
use crypto_art::log::*;

use ringbuffer::{
    RingBuffer,
    RingBufferExt,
    RingBufferRead,
    RingBufferWrite
};

use js_sys::SharedArrayBuffer;
use crypto_art::mem::{
    DataView,
    SharedRingBuffer,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct TestObject {
    value: u32
}

#[wasm_bindgen_test]
fn test_shared_data_view() {
    let shared: SharedArrayBuffer = SharedArrayBuffer::new(64);
    let view: DataView = DataView::new(&shared, 0, 2);

    view.set_uint8(0, 1);
    view.set_uint8(1, 255);

    assert_eq!(view.get_uint8(0), 1);
    assert_eq!(view.get_uint8(1), 255);
}

#[wasm_bindgen_test]
fn test_ring_buffer_enqueue_dequeue() {
    let mut srb: SharedRingBuffer<TestObject> = SharedRingBuffer::new(64);
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