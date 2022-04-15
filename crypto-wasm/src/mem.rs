extern crate alloc;

use wasm_bindgen::prelude::*;

use alloc::{
    string::String,
    vec::Vec
};

use core::{
    borrow::Borrow,
    convert::{
        From,
        Into
    },
    mem::{
        size_of
    },
    marker::{
        PhantomData,
        Sized,
        Copy
    },
    option::Option,
    ops::{
        Deref,
        Range
    }
};

use js_sys::{
    SharedArrayBuffer,
    Uint8Array,
    Atomics,
    Object
};

use ringbuffer::{
    RingBuffer,
    RingBufferExt,
    RingBufferRead,
    RingBufferWrite
};

use serde::{Serialize, Deserialize, de::DeserializeOwned};
use serde_cbor::{
    Serializer,
    Deserializer,
    ser::to_vec,
    de::from_mut_slice
};

#[macro_use]
use crate::log::*;

// TODO: Implement a Slice struct that contains the offset and a trait with helper methods to get correct bit offsets
const LENGTH_BIT_U16: usize = 16; // Additional length added to each chunk
const LENGTH_BIT_LOCK: usize = 8; // Additional bit to be used for locking access to the Buffer
const START_INDEX: usize = (LENGTH_BIT_U16 + LENGTH_BIT_LOCK) / 8;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = Object, typescript_type = "DataView", js_name = "DataView")]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub type DataView;

    #[wasm_bindgen(constructor)]
    pub fn new(buffer: &SharedArrayBuffer, byteOffset: usize, byteLength: usize) -> DataView;

    #[wasm_bindgen(method, getter, structural)]
    pub fn buffer(this: &DataView) -> SharedArrayBuffer;

    #[wasm_bindgen(method, getter, structural, js_name = byteLength)]
    pub fn byte_length(this: &DataView) -> usize;

    #[wasm_bindgen(method, getter, structural, js_name = byteOffset)]
    pub fn byte_offset(this: &DataView) -> usize;

    #[wasm_bindgen(method, js_name = getUint8)]
    pub fn get_uint8(this: &DataView, byte_offset: usize) -> u8;

    #[wasm_bindgen(method, js_name = setUint8)]
    pub fn set_uint8(this: &DataView, byte_offset: usize, value: u8);

    #[wasm_bindgen(method, js_name = getUint16)]
    pub fn get_uint16(this: &DataView, byte_offset: usize) -> u16;

    #[wasm_bindgen(method, js_name = setUint16)]
    pub fn set_uint16(this: &DataView, byte_offset: usize, value: u16);
}

const IS_LOCKED: i32 = 1 << 0;

pub struct SharedRingBuffer<T: ?Sized + Serialize + DeserializeOwned> {
    raw: SharedArrayBuffer,
    head: usize,
    tail: usize,
    length: usize,
    capacity: usize,
    raw_capacity: usize, // Cache to prevent constantly converting
    slice_size: usize,
    marker: PhantomData<T>
}

impl<T: ?Sized + Serialize + DeserializeOwned> SharedRingBuffer<T> {
    pub fn new(len: usize) -> Self {
        let slice_size: usize = (size_of::<T>() * 8) + LENGTH_BIT_U16; // 16 bits for size storage
        let raw_len: usize = (len * slice_size) + LENGTH_BIT_LOCK;

        let raw: SharedArrayBuffer = SharedArrayBuffer::new(raw_len as u32);
        Atomics::store(&raw, 0, IS_LOCKED);

        return SharedRingBuffer {
            //TODO: Clean up this cast
            raw: raw,
            head: 0,
            tail: 0,
            length: 0,
            capacity: len,
            raw_capacity: raw_len,
            slice_size: slice_size,
            marker: PhantomData
        };
    }
}

impl<T: ?Sized + Serialize + DeserializeOwned> RingBuffer<T> for SharedRingBuffer<T> {
    fn len(&self) -> usize {
        return self.length;
    }

    fn capacity(&self) -> usize {
        return self.capacity;
    }
}

impl<A: ?Sized + Serialize + DeserializeOwned> Extend<A> for SharedRingBuffer<A> {
    fn extend<T: IntoIterator<Item = A>>(&mut self, iter: T) {
        for elem in iter {
            self.push(elem);
        }
    }
}

/* 
    We use asserts within these function definitions as it is better to Panic than allow the ringbuffer to get corrupt
*/
impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferWrite<T> for SharedRingBuffer<T> {
    fn push(&mut self, value: T) {
        assert!(self.length < self.capacity());

        let new_head: usize = (self.head + self.slice_size) % self.raw_capacity;
        assert!(new_head > self.tail, "Head of buffer would overwrite tail, what are you some kind of Ouroboros?");

        let data_view: DataView = DataView::new(&self.raw, self.head, self.slice_size);
        let serialized: Vec<u8> = to_vec(&value).expect("Unable to serialize value");

        assert!(serialized.len() <= self.slice_size - LENGTH_BIT_U16, "Serialized value exceeds buffer size");

        // Set size
        data_view.set_uint16(0, serialized.len() as u16);

        let mut index = START_INDEX;
        for u8_byte in serialized {
            data_view.set_uint8(index, u8_byte);
            index += 1;
        }

        self.length += 1;
        self.head = new_head;
    }
}

impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferRead<T> for SharedRingBuffer<T> {
    fn dequeue(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let new_tail: usize = (self.tail + self.slice_size) % self.raw_capacity;
        // Inclusive <= as the tail can consume the head bytes
        assert!(new_tail <= self.head, "Tail of buffer would overwrite head");
    
        // TODO: Investigate if 
        //let view: Uint8Array = Uint8Array::new_with_byte_offset_and_length(&self.raw, self.tail as u32, self.slice_size as u32);

        let data_view: DataView = DataView::new(&self.raw, self.tail, self.slice_size);
        let mut buffer: Vec<u8> = Vec::with_capacity(self.slice_size);

        let size: usize = data_view.get_uint16(0) as usize + START_INDEX;

        for i in START_INDEX..size {
            buffer.push(data_view.get_uint8(i));
        }

        let result: T = from_mut_slice(&mut *buffer).expect("Unable to deserialize object");

        self.length -= 1;
        self.tail = new_tail;

        return Some(result);
    }

    fn skip(&mut self) {
        self.dequeue();
    }
}

/*impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferExt<T> for SharedRingBuffer<T> {
    fn fill_with<F: FnMut() -> T>(&mut self, f: F) {
        for i in 0..self.capacity() - 1 {
            self.push(f());
        }
    }

    /// Empties the buffer entirely. Sets the length to 0 but keeps the capacity allocated.
    fn clear(&mut self) {
        let view: Uint8Array = Uint8Array::new(&self.raw);
        view.fill(0, 1, self.raw_capacity as u32);

        self.head = 0;
        self.tail = 0;
        self.length = 0;
    }

    /// Gets a value relative to the current index. 0 is the next index to be written to with push.
    /// -1 and down are the last elements pushed and 0 and up are the items that were pushed the longest ago.
    fn get(&self, index: isize) -> Option<&T> {

    }

    /// Gets a value relative to the current index mutably. 0 is the next index to be written to with push.
    /// -1 and down are the last elements pushed and 0 and up are the items that were pushed the longest ago.
    fn get_mut(&mut self, index: isize) -> Option<&mut T> {

    }

    /// Gets a value relative to the start of the array (rarely useful, usually you want [`Self::get`])
    fn get_absolute(&self, index: usize) -> Option<&T> {

    }

    /// Gets a value mutably relative to the start of the array (rarely useful, usually you want [`Self::get_mut`])
    fn get_absolute_mut(&mut self, index: usize) -> Option<&mut T> {

    }
}*/