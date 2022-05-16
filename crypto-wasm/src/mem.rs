use wasm_bindgen::prelude::*;

use core::{
    borrow::Borrow,
    convert::From,
    convert::Into,
    iter::FromIterator,
    mem::size_of,
    marker::PhantomData,
    marker::Sized,
    marker::Copy,
    option::Option,
    ops::Deref,
    ops::DerefMut,
    ops::Range,
    ops::IndexMut,
    ops::Index,
    convert::AsMut,
    convert::AsRef
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
    ser::Serializer,
    ser::SliceWrite,
    de::from_mut_slice
};

use bumpalo::{
    Bump,
    collections::Vec as BumpVec,
    collections::CollectionAllocErr
};

use crate::log::*;
use crate::sync::Arc;

// TODO: Implement a Slice struct that contains the offset and a trait with helper methods to get correct bit offsets
const LENGTH_BIT_U16: usize = 16; // Additional length added to each chunk
const LENGTH_BIT_LOCK: usize = 8; // Additional bit to be used for locking access to the Buffer
const START_INDEX: usize = LENGTH_BIT_U16 / 8;

// 5 byte major value 30 is reserved/not for use, used to denote empty u8 cell
const BLANK_CBOR_TAG: u8 = 30 << 3;

const CELL_SIZE_BYTES: usize = size_of::<AllocatorCell>();

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

    #[wasm_bindgen(method, js_name = getUint32)]
    pub fn get_uint32(this: &DataView, byte_offset: usize) -> u32;

    #[wasm_bindgen(method, js_name = setUint32)]
    pub fn set_uint32(this: &DataView, byte_offset: usize, value: u32);
}

#[derive(Debug, Clone)]
pub struct AllocatorPoolError<'a> {
    pub reason: &'a str
}

impl<'a> core::fmt::Display for AllocatorPoolError<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        return write!(f, "Invalid AllocatorPool Operation: {}", self.reason);
    }
}

#[derive(Debug, Clone)]
pub struct AllocatorPool<'a> {
    root_alloc: &'a Bump,
    allocators: BumpVec<'a, AllocatorCell>,
}

#[derive(Debug, Clone)]
pub struct AllocatorCell {
    allocator: Arc<Bump>
}

impl AllocatorCell {
    pub fn new(root_allocator: &Bump, allocator: Bump) -> Self {
        return Self {
            allocator: Arc::new_in(root_allocator, allocator)
        }
    }

    pub fn get_mut(&mut self) -> Option<&mut Bump> {
        if self.allocator.ref_count() <= 2 {
            return unsafe { Some(self.allocator.get_mut_unchecked()) };
        }

        return None;
    }
}

impl Deref for AllocatorCell {
    type Target = Arc<Bump>;

    fn deref(&self) -> &Arc<Bump> {
        return &self.allocator;
    }
}

impl Drop for AllocatorCell {
    fn drop(&mut self) {
        if self.allocator.ref_count() <= 2 {
            unsafe { self.allocator.get_mut_unchecked().reset() };
        }
    }
}

// This probably leaks memory over time, lmao. Really could do with a proper arena or allocator.
// As long as someone doesn't add an absolute ton of allocators, we won't have too many dead,
// long lived AllocatorCell References on the root_alloc heap
impl<'a> AllocatorPool<'a> {
    pub fn new(root_alloc: &'a Bump) -> Self {
        let allocators: BumpVec<'a, AllocatorCell> = BumpVec::new_in(root_alloc);

        return AllocatorPool {
            root_alloc: root_alloc,
            allocators: allocators
        }
    }

    pub fn new_with_init<T>(root_alloc: &'a Bump, num_allocators: usize, length: usize) -> Self {
        assert!(length <= root_alloc.chunk_capacity() / CELL_SIZE_BYTES);

        let mut allocators: BumpVec<'a, AllocatorCell> = BumpVec::with_capacity_in(num_allocators, &root_alloc);

        // fill_with doesn't seem to work for some reason, perhaps because it's capacity is f'ed idk why
        for i in 0..num_allocators {
            allocators.insert(i, AllocatorCell::new(root_alloc, Bump::with_capacity(size_of::<T>() * length)));
        }

        return AllocatorPool {
            root_alloc: root_alloc,
            allocators: allocators
        }
    }

    pub fn create_bumpalo<T>(length: usize) -> Bump {
        return Bump::with_capacity(size_of::<T>() * length);
    }

    pub fn expand(&mut self, length: usize) -> Result<(), CollectionAllocErr> {
        return self.allocators.try_reserve(length);
    }

    pub fn shrink(&mut self, length: usize) {
        self.allocators.truncate(length);
        self.allocators.shrink_to_fit();
    }

    pub fn initialize<T>(&mut self, index: usize, length: usize) {
        self.allocators.insert(index, AllocatorCell::new(self.root_alloc, Bump::with_capacity(size_of::<T>() * length)));
    }

    pub fn get(&self, index: usize) -> AllocatorCell {
        return self.allocators.get(index).expect(format!("No allocator at specified index {:#}", index).as_str()).clone();
    }

    pub fn get_ref(&self, index: usize) -> &AllocatorCell {
        return self.allocators.get(index).expect(format!("No allocator at specified index {:#}", index).as_str());
    }

    pub fn get_mut(&mut self, index: usize) -> &mut AllocatorCell {
        return self.allocators.get_mut(index).expect(format!("No allocator at specified index {:#}", index).as_str());
    }

    pub fn has(&self, index: usize) -> bool {
        return self.allocators.get(index).is_some();
    }

    pub fn clear(&mut self, index: usize) -> Result<(), AllocatorPoolError<'a>> {
        if let Some(cell) = self.allocators.get_mut(index) {
            if let Some(allocator) = cell.allocator.get_mut() {
                allocator.reset();
                return Ok(());
            }

            return Err(AllocatorPoolError{
                reason: "Unable to acquire mut ref for allocator at index"
            });
        }

        return Err(AllocatorPoolError{
            reason: "No allocator found at index"
        });
    }

    pub fn len(&self) -> usize {
        return self.allocators.len();
    }

    pub fn capacity(&self) -> usize {
        return self.allocators.capacity();
    }
}

#[derive(Debug, Clone)]
pub struct RingBufferError<'a> {
    pub reason: &'a str
}

impl<'a> core::fmt::Display for RingBufferError<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        return write!(f, "Invalid RingBuffer Operation: {}", self.reason);
    }
}

pub struct SharedRingBuffer<T: ?Sized + Serialize + DeserializeOwned> {
    scratch: AllocatorCell,
    raw: SharedArrayBuffer,
    view: DataView,
    head: usize,
    tail: usize,
    length: usize,
    capacity: usize,
    raw_capacity: usize, // Cache to prevent constantly converting
    slice_size: usize,
    marker: PhantomData<T>
}

impl<T: ?Sized + Serialize + DeserializeOwned> SharedRingBuffer<T> {
    pub fn new(root_allocator: &Bump, len: usize) -> Self {
        let slice_size: usize = (size_of::<T>() * 8) + LENGTH_BIT_U16; // 16 bits for size storage
        let view_len: usize = len * slice_size;

        // + additional bit for mutex
        let raw: SharedArrayBuffer = SharedArrayBuffer::new((view_len + LENGTH_BIT_LOCK) as u32);

        return SharedRingBuffer {
            scratch: AllocatorCell::new(root_allocator, Bump::with_capacity(view_len)),
            view: DataView::new(&raw, 1, view_len),
            raw: raw,
            head: 0,
            tail: 0,
            length: 0,
            capacity: len,
            raw_capacity: view_len,
            slice_size: slice_size,
            marker: PhantomData
        };
    }

    // Read a value at an index without consuming it
    pub fn read(&self, index: usize) -> Option<T> {
        let local_scratch: AllocatorCell = self.scratch.clone();
        let mut buffer: BumpVec<u8> = bumpalo::vec![in &local_scratch; BLANK_CBOR_TAG; self.slice_size];

        match self.read_into(index, buffer.as_mut_slice()) {
            Ok(size) => {
                if size == 0 {
                    return None;
                }
                buffer.truncate(size);
                return from_mut_slice(buffer.as_mut_slice()).ok();
            },
            Err(_) => return None
        }
    }

    pub fn read_into<'a>(&self, index: usize, scratch: &'a mut [u8]) -> Result<usize, RingBufferError> {
        if index > self.capacity - 1 {
            return Err(RingBufferError{ reason: "RingBuffer is full!" });
        }

        if scratch.len() < self.slice_size {
            return Err(RingBufferError{ reason: "Scratch space provided potentially not large enough to store deserialized bytes!" });
        }

        let real_index = index * self.slice_size;
        let size: usize = self.view.get_uint16(real_index) as usize;

        let mut scratch_index: usize = 0;
        for i in real_index + START_INDEX..size + real_index + START_INDEX {
            scratch[scratch_index] = self.view.get_uint8(i);
            scratch_index += 1;
        }

        return Ok(scratch_index);
    }

    // Write a value at a particular index
    pub fn write(&mut self, value: &T, index: usize) {
        assert!(index < self.capacity);

        let local_scratch: AllocatorCell = self.scratch.clone();
        let mut buffer: BumpVec<u8> = bumpalo::vec![in &local_scratch; BLANK_CBOR_TAG; self.slice_size];
        let mut serializer = Serializer::new(SliceWrite::new(buffer.as_mut_slice()));

        value.serialize(&mut serializer).expect("Unable to serialize value");
        //assert!(serialized.len() <= self.slice_size - LENGTH_BIT_U16, "Serialized value exceeds max slot size");

        let mut size: u16 = 0;
        let mut real_index = (index * self.slice_size) + START_INDEX;
        for i in 0..buffer.len() - 1 {
            if buffer[i] == BLANK_CBOR_TAG {
                break;
            }

            self.view.set_uint8(real_index, buffer[i]);
            real_index += 1;
            size += 1;
        }

        drop(buffer);

        self.view.set_uint16(index * self.slice_size, size);
    }

    pub fn get_raw(&self, index: isize) -> Option<T> {
        let _index: Option<usize> = self.parse_index(index);
        match _index {
            Some(i) => return self.read(i),
            None => return None
        }
    }

    pub fn get_absolute_raw(&self, index: usize) -> Option<T> {
        return self.read(index);
    }

    // Could possibly do this with a Binary NOT too
    pub fn parse_index(&self, index: isize) -> Option<usize> {
        if index.abs() > self.length as isize {
            return None;
        }

        let offset: isize = self.head as isize + index;

        if offset < 0 {
            return Some(self.capacity - offset.abs() as usize);
        }

        return Some(offset as usize);
    }

    pub fn get_head_index(&self) -> usize {
        return self.head;
    }

    pub fn get_tail_index(&self) -> usize {
        return self.tail;
    }

    pub fn uint8_view(&self) -> Uint8Array {
        return Uint8Array::new(&self.raw);
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
    fn extend<T: IntoIterator<Item = A>>(&mut self, _: T) {
        unimplemented!("Unable to extend fixed array bounds!");
    }
}

impl <T: ?Sized + Serialize + DeserializeOwned> FromIterator<T> for SharedRingBuffer<T> {
    fn from_iter<I: IntoIterator<Item=T>>(iter: I) -> Self {
        unimplemented!("Cba to implement right now!");

        /*let items: Vec<T> = iter.into_iter().collect();
        let mut ringbuffer: SharedRingBuffer<T> = SharedRingBuffer::new(items.len());

        for item in items {
            ringbuffer.push(item);
        }

        return ringbuffer;*/
    }
}

impl <T: ?Sized + Serialize + DeserializeOwned> Index<isize> for SharedRingBuffer<T> {
    type Output = T;

    fn index(&self, index: isize) -> &Self::Output {
        return self.get(index).expect("Invalid index specified");
    }
}

impl <T: ?Sized + Serialize + DeserializeOwned> IndexMut<isize> for SharedRingBuffer<T> {
    fn index_mut(&mut self, index: isize) -> &mut Self::Output {
        return self.get_mut(index).expect("Invalid index specified");
    }
}

// TODO: Reimplement traits with Result<()>
impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferWrite<T> for SharedRingBuffer<T> {
    fn push(&mut self, value: T) {
        assert!(self.length < self.capacity());

        self.write(&value, self.head);

        self.length += 1;
        self.head = (self.head + 1) % self.capacity;
    }
}

// TODO: Reimplement traits with Result<()>
impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferRead<T> for SharedRingBuffer<T> {
    fn dequeue(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }

        let old_tail: usize = self.tail;
        let result: Option<T> = self.read(self.tail);

        // Tombstone the block, so it cannot be read again
        self.view.set_uint16(old_tail * self.slice_size, 0);

        self.length -= 1;
        self.tail = (self.tail + 1) % self.capacity;

        return result;
    }

    fn skip(&mut self) {
        self.dequeue();
    }
}

impl<T: ?Sized + Serialize + DeserializeOwned> RingBufferExt<T> for SharedRingBuffer<T> {
    fn fill_with<F: FnMut() -> T>(&mut self, mut f: F) {
        self.head = 0;
        self.tail = 0;
        self.length = 0;

        for _ in 0..self.capacity {
            self.push(f());
        }
    }

    fn clear(&mut self) {
        self.head = 0;
        self.tail = 0;
        self.length = 0;

        for i in 0..self.raw_capacity / 32 {
            self.view.set_uint32(i, 0);
        }
    }

    /*
        Compatability layer, prefer that the user directly calls read_into
    */
    fn get(&self, index: isize) -> Option<&T> {
        let _index: Option<usize> = self.parse_index(index);
        match _index {
            Some(i) => return self.get_absolute(i),
            None => return None
        }
    }

    fn get_mut(&mut self, index: isize) -> Option<&mut T> {
        let _index: Option<usize> = self.parse_index(index);
        match _index {
            Some(i) => return self.get_absolute_mut(i),
            None => return None
        }
    }

    // TODO: Make these return some form of Boxed result that autodrops?
    fn get_absolute(&self, index: usize) -> Option<&T> {
        match self.read(index) {
            Some(result) => {
                return Some(self.scratch.alloc(result));
            }
            None => return None
        }
    }

    fn get_absolute_mut(&mut self, index: usize) -> Option<&mut T> {
        match self.read(index) {
            Some(result) => {
                return Some(self.scratch.alloc(result));
            }
            None => return None
        }
    }
}