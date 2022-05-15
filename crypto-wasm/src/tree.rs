extern crate alloc;

use async_trait::async_trait;

use bumpalo::{
    Bump,
    collections::Vec as BumpVec
};

use core::{
    usize,
    mem,
    cmp::Ord,
    cmp::Ordering,
    cmp::PartialOrd,
    cmp::Eq,
    cmp::PartialEq,
    iter::Iterator,
    default::Default,
    borrow::Borrow,
    marker::Sync,
    marker::PhantomData,
    sync::atomic::AtomicUsize,
    sync::atomic::Ordering as AtomicOrdering,
    task::Poll,
    task::Context,
    future::Future,
    pin::Pin,
    ops::Deref
};

use crate::sync::Arc;
use crate::mem::{
    AllocatorPool,
    AllocatorCell
};

use crate::ecdh::{
    Key
};
use crate::errors::RatchetError;

use hashbrown::{
    HashSet,
    HashMap,
    hash_map
};

// Used to count number of active branches so we can 
static BRANCH_COUNT: AtomicUsize = AtomicUsize::new(0);

const MEMORY_ROOT_NODE_INDEX: usize = 0;
const MEMORY_ORPHAN_NODE_INDEX: usize = 1;
const MEMORY_BRANCH_INDEX: usize = 2;
const MEMORY_TREE_START_INDEX: usize = 3;

pub fn is_even(i: usize) -> bool {
    return i & 0x1 == 0;
}

pub fn lsb(i: usize) -> usize {
    let _i: isize = i as isize;
    return (_i & (-_i)) as usize;
}

pub fn get_next_index(i: usize) -> usize {
    return (i + (i & 0x1)) / 2;
}

/*
* Get the sibling pair index for key at index i
* get_sibling_index(1) = 2
* get_sibling_index(2) = 1
* And so on and so forth...
*/
pub fn get_sibling_index(i: usize) -> usize {
    if is_even(i) {
        return i - 1
    }

    return i + 1
}

pub trait TreeOperations {
    fn search(&self);
    fn ratchet(&mut self, index: usize) -> Result<&Key, RatchetError>;
    fn insert(&mut self, key: &Key) -> Result<&Key, RatchetError>;
}

// TODO: Implement Clone/Copy for tree cache
//#[derive(Debug)]
pub struct RatchetTree<'a> {
    memory: &'a AllocatorPool<'a>,
    nodes: BumpVec<'a, BumpVec<'a, Key>>,
    orphans: BumpVec<'a, usize>
}

pub struct RatchetBranch<'a> {
    root: usize,
    nodes: BumpVec<'a, Key>
}

pub struct RatchetIter {
    index: usize,
    height: usize,
    curr_index: usize,
    curr_height: usize
}

impl RatchetIter {
    pub fn new(index: usize, height: usize, curr_height: usize) -> Self {
        return Self {
            index: index,
            height: height,
            curr_index: index,
            curr_height: curr_height
        };
    }

    pub fn reset(&mut self) {
        self.curr_index = self.index;
    }
}

impl<'a> RatchetBranch<'a> {
    fn new(allocator: &'a AllocatorCell, root: usize) -> Self {        
        return Self {
            root: root,
            nodes: BumpVec::new_in(allocator)
        }
    }

    fn add_node(&mut self, index: usize, key: Key) {
        self.nodes.insert(index, key);
    }

    fn get_node(&self, index: &usize) -> Option<&Key> {
        return self.nodes.get(*index);
    }

    fn iter(&self) -> core::slice::Iter<Key> {
        return self.nodes.iter();
    }

    fn len(&self) -> usize {
        return self.nodes.len();
    }

    fn clear(&mut self) {
        self.nodes.clear();
    }
}

impl Iterator for RatchetIter {
    type Item = (usize, usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        /*
        * This if block functions as two things:
        * First of all, it prevents usage of any index value < 0, as it's a fenwick tree
        * Secondly, it ensures the iterator properly exits once we no longer have at least (2) key pairs to DH with
        */
        if self.curr_index < 2 && self.curr_height >= self.height {
            return None;
        }

        let next: Self::Item = (self.curr_height, self.curr_index, get_sibling_index(self.index));

        self.curr_index = get_next_index(self.index);
        self.curr_height += 1;

        return Some(next);
    }
}

/*
* This data structure perhaps makes some naiive assumptions: We assume that it is not possible to remove
* the first index (0) in the nodes array. TODO: Make it impossible to do such with a wrapper object around Vec?
*
* Memory is provided via multiple Bump allocators, each tied to a specific role in the tree. This allows for
* cleaner segmentation of memory and makes each layer of the tree droppable so memory can be freed.
* Memory Pool indexes:
* - Root leaf Nodes: 0
* - Orphan Node vec: 1
* - Ratchet Branch  vec: 2
* - Layer 1 - 3
* - Layer 2 - 4
* ... and so forth
*
*/
impl<'a> RatchetTree<'a> {
    pub fn new(memory: &'a mut AllocatorPool) -> Result<Self, RatchetError<'a>> {
        let mut nodes: BumpVec<BumpVec<Key>> = BumpVec::with_capacity_in(1, memory.get_ref(MEMORY_ROOT_NODE_INDEX));
        nodes.insert(0, BumpVec::with_capacity_in(1, memory.get_ref(MEMORY_ORPHAN_NODE_INDEX)));

        return Ok(
            Self {
                memory: memory,
                nodes: nodes,
                orphans: BumpVec::with_capacity_in(1, memory.get_ref(MEMORY_BRANCH_INDEX))
            }
        );
    }

    pub fn get_next_index(&mut self) -> usize {
        match self.orphans.pop() {
            Some(orphan) => return orphan,
            None => return self.nodes[0].len() + 1
        }
    }

    pub fn height(&self) -> usize {
        return (self.nodes[0].len() as f64).log(2.0).ceil() as usize;
    }

    pub fn iter(&self, index: usize) -> RatchetIter {
        return RatchetIter::new(index, self.height(), 0);
    }

    /*pub fn ratchet(&mut self, index: usize, key: &Key) -> Result<RatchetBranch, RatchetError> {
        let mut iterator: RatchetIter = self.iter(index);
        let mut branch: RatchetBranch = RatchetBranch::new(
            &self.memory.get(RatchetMemoryLayout::BRANCH_MEMORY.into()),
            index
        );

        // Root of the branch is our node
        branch.add_node(index, *key);

        // Two phase commit: parity check to ensure values are present before committing
        while let Some(key_tuple) = iterator.next() {
            let height: usize = key_tuple.0;

            if let Some(layer) = self.nodes.get(height) {
                let k1: Option<&Key> = layer.get(key_tuple.1); // Key 1
                let k2: Option<&Key> = layer.get(key_tuple.2); // Key 2

                if k1.is_none() || k2.is_none() {
                    return Err(RatchetError{
                        reason: &format_args!("Cannot diffie-hellman: Keypair is None at height {:#}", height).as_str().unwrap(),
                        index: key_tuple.1,
                        height: height
                    });
                }

                let res: Result<Key, crate::errors::ECError> = k1.unwrap().diffie_hellman(k2.unwrap());

                match res {
                    Ok(key) => branch.add_node(iterator.curr_index, key),
                    Err(e) => {
                        return Err(RatchetError{
                            reason: &format_args!("Diffie hellman failed: {}", e.reason).as_str().unwrap(),
                            index: key_tuple.1,
                            height: height
                        });
                    }
                }
            }
        }

        return Ok(branch);
    }

    pub fn commit(&'a mut self, branch: &RatchetBranch) -> Result<&Key, RatchetError> {
        if branch.len() < self.height() - 1 { // Omit top most node in height, as it is the Tree Secret
            return Err(RatchetError{
                reason: "Branch & Tree height mismatch: Committing branch would result in desynced state",
                index: branch.root,
                height: branch.len()
            });
        }

        let mut iter: core::slice::Iter<Key> = branch.iter();
        let mut height: usize = 0;
        let mut index: usize = branch.root;

        while let Some(node) = iter.next() {
            if self.nodes.get(height).is_none() {
                self.nodes.insert(height, &BumpVec::<'a, Key>::with_capacity_in(1, &self.memory.y_allocator)); 
            }

            self.nodes[height][index] = *node;
            height += 1;
            index += 1;
        }

        return Ok(&self.nodes[height][1]);
    }*/

    /*// Do not immediately commit the key, return a commit view so we can commit on txn confirmation
    pub fn insert(&mut self, key: &Key) -> Result<RatchetBranch, RatchetError> {
        return self.ratchet(self.get_next_index(), key);
    }

    pub fn rebalance() {
        return;
    }*/
}