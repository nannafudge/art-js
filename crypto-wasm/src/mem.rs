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

const LENGTH_BIT_U16: usize = 16;
const START_INDEX: usize = LENGTH_BIT_U16 / 8;

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

pub struct SharedRingBuffer<T: ?Sized + Serialize + DeserializeOwned> {
    raw: SharedArrayBuffer,
    raw_capacity: usize, // Cache to prevent constantly converting

    obj_size: usize,
    length: usize,
    head: usize,
    tail: usize,
    marker: PhantomData<T>
}

impl<T: ?Sized + Serialize + DeserializeOwned> SharedRingBuffer<T> {
    pub fn new(len: usize) -> Self {
        let _obj_size: usize = (size_of::<T>() * 8) + LENGTH_BIT_U16; // 16 bits for size storage
        let _raw_len: usize = len * _obj_size;

        return SharedRingBuffer {
            //TODO: Clean up this cast
            raw_capacity: _raw_len,
            raw: SharedArrayBuffer::new(_raw_len as u32),
            obj_size: _obj_size,
            length: 0,
            head: 0,
            tail: 0,
            marker: PhantomData
        };
    }
}

impl<T: ?Sized + Serialize + DeserializeOwned> RingBuffer<T> for SharedRingBuffer<T> {
    fn len(&self) -> usize {
        return self.length;
    }

    fn capacity(&self) -> usize {
        return self.raw.byte_length() as usize / self.obj_size;
    }
}

impl<A: ?Sized + Serialize + DeserializeOwned> Extend<A> for SharedRingBuffer<A> {
    fn extend<T: IntoIterator<Item = A>>(&mut self, iter: T) {
        for elem in iter {
            self.push(elem);
        }
    }
}

impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferWrite<T> for SharedRingBuffer<T> {
    fn push(&mut self, value: T) {
        assert!(self.length < self.capacity());

        let new_head: usize = (self.head + self.obj_size) % self.raw_capacity;
        assert!(new_head > self.tail, "Head of buffer would overwrite tail, what are you some kind of Ouroboros?");

        let data_view: DataView = DataView::new(&self.raw, self.head, new_head);
        let serialized: Vec<u8> = to_vec(&value).expect("Unable to serialize value");

        assert!(serialized.len() <= self.obj_size - LENGTH_BIT_U16, "Serialized value exceeds buffer size");

        // Set size
        data_view.set_uint16(0, serialized.len() as u16);

        let mut index = START_INDEX;
        for u8_byte in serialized {
            info!("{a}: {b}", a=index, b=u8_byte);

            data_view.set_uint8(index, u8_byte);
            index += 1;
        }

        info!("New head is {}", new_head);

        self.length += 1;
        self.head = new_head;
    }
}

impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferRead<T> for SharedRingBuffer<T> {
    fn dequeue(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let new_tail: usize = (self.tail + self.obj_size) % self.raw_capacity;
        assert!(new_tail <= self.head, "Tail of buffer would overwrite head");
    
        let data_view: DataView = DataView::new(&self.raw, self.tail, new_tail);
        let mut buffer: Vec<u8> = Vec::with_capacity(self.obj_size);

        let size: usize = data_view.get_uint16(0) as usize + START_INDEX;

        for i in START_INDEX..size {
            let u8_byte: u8 = data_view.get_uint8(i);

            info!("{a}: {b}", a=i, b=u8_byte);
            buffer.push(u8_byte);
        }

        let result: T = from_mut_slice(&mut *buffer).expect("Unable to deserialize object");

        info!("New tail is {}", new_tail);

        self.length -= 1;
        self.tail = new_tail;

        return Some(result);
    }

    fn skip(&mut self) {
        self.dequeue();
    }
}