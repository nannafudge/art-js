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
use crate::log::*;

use hashbrown::{
    HashSet,
    HashMap,
    hash_map
};

pub const MEMORY_ROOT_NODE_INDEX: usize = 0;
pub const MEMORY_ORPHAN_NODE_INDEX: usize = 1;
pub const MEMORY_BRANCH_INDEX: usize = 2;
pub const MEMORY_TREE_START_INDEX: usize = 3;

pub fn is_even(i: usize) -> bool {
    return i & 0x1 == 0;
}

pub fn lsb(i: usize) -> usize {
    let _i: isize = i as isize;
    return (_i & (-_i)) as usize;
}

// Function rounds **up** an odd number to the next greatest even number
// number remains unchanged if already even
pub fn round_up(i: usize) -> usize {
    return i + (i & 0x1);
}

pub fn get_next_index(i: usize) -> usize {
    return round_up(i) / 2;
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
    orphans: BumpVec<'a, usize>,
    pub tombstone: Option<Key>
}

pub struct RatchetBranch<'a> {
    pub root: usize,
    pub nodes: BumpVec<'a, Key>
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
    fn new(allocator_ref: &'a AllocatorCell, root: usize) -> Self {        
        return Self {
            root: root,
            nodes: BumpVec::new_in(allocator_ref)
        }
    }

    pub fn add_node(&mut self, key: Key) {
        self.nodes.push(key);
    }

    pub fn get_node(&self, index: usize) -> Option<&Key> {
        return self.nodes.get(index);
    }

    pub fn get_last(&self) -> Option<&Key> {
        if self.len() == 0 {
            return None;
        }

        return self.nodes.get(self.len() - 1);
    }

    pub fn iter(&self) -> core::slice::Iter<Key> {
        return self.nodes.iter();
    }

    pub fn len(&self) -> usize {
        return self.nodes.len();
    }

    pub fn clear(&mut self) {
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

        let next: Self::Item = (self.curr_height, self.curr_index, get_sibling_index(self.curr_index));

        self.curr_index = get_next_index(self.curr_index);
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
impl<'tree> RatchetTree<'tree> {
    pub fn new(memory: &'tree mut AllocatorPool) -> Self {
        assert!(memory.capacity() >= 4);

        let mut nodes: BumpVec<BumpVec<Key>> = BumpVec::with_capacity_in(16, memory.get_ref(MEMORY_ROOT_NODE_INDEX));
        let mut first_layer: BumpVec<Key> = BumpVec::new_in(memory.get_ref(MEMORY_TREE_START_INDEX));

        first_layer.insert(0, Key::default());
        nodes.insert(0, first_layer);

        return Self {
            memory: memory,
            nodes: nodes,
            orphans: BumpVec::new_in(memory.get_ref(MEMORY_ORPHAN_NODE_INDEX)),
            tombstone: Some(Key::default())
        }
    }

    pub fn get_next_index(&self) -> usize {
        match self.orphans.get(0) {
            Some(orphan) => return *orphan,
            None => return self.nodes[0].len()
        }
    }

    pub fn height(&self) -> usize {
        let node_len: usize = self.nodes[0].len() - 1;
        return if node_len == 0 { 0 } else { (node_len as f64).log(2.0).ceil() as usize };
    }

    pub fn iter(&self, index: usize) -> RatchetIter {
        return RatchetIter::new(index, self.height(), 0);
    }

    pub fn ensure_layer_present(&mut self, height: usize, memory: &'tree AllocatorCell) {
        if self.nodes.get(height).is_none() {
            let mut layer: BumpVec<Key> = BumpVec::new_in(memory);
            layer.insert(0, Key::default());

            self.nodes.insert(height, layer);
        }
    }

    pub fn ratchet<'caller>(tree: &Self, index: usize, key: &Key, scratch: &'caller AllocatorCell) -> Result<RatchetBranch<'caller>, RatchetError<'caller>> {
        let mut iterator: RatchetIter = tree.iter(index);
        let mut branch: RatchetBranch = RatchetBranch::new(
            scratch,
            index
        );

        // Root of the branch is our node
        branch.add_node(*key);

        // Two phase commit
        while let Some(key_tuple) = iterator.next() {
            let height: usize = key_tuple.0;

            if let Some(layer) = tree.nodes.get(height) {
                // Seed Key1 from previous DH result, if available
                let k1: Option<&Key> = branch.get_last();
                let k2: Option<&Key> = layer.get(key_tuple.2); // Key 2

                let no_key1: bool = k1.is_none() || k1 == tree.tombstone.as_ref();
                let no_key2: bool = k2.is_none() || k2 == tree.tombstone.as_ref();

                if no_key1 && no_key2 {
                    branch.add_node(tree.tombstone.unwrap());
                    continue;
                }

                if !no_key1 && no_key2 {
                    branch.add_node(*k1.unwrap());
                    continue;
                }

                if no_key1 && !no_key2 {
                    branch.add_node(*k2.unwrap());
                    continue;
                }

                // I don't implicitly convert into an Option<Key> here because I want to explicitly
                // warn of a diffie-hellman failure
                let res: Result<Key, crate::errors::ECError> = k1.unwrap().diffie_hellman(k2.unwrap());

                match res {
                    Ok(key) => branch.add_node(key),
                    Err(_) => {
                        return Err(RatchetError{
                            reason: "Diffie hellman failed",
                            index: key_tuple.1,
                            height: height
                        });
                    }
                }
            }
        }

        return Ok(branch);
    }

    pub fn commit<'caller>(tree: &'caller mut Self, branch: &'caller RatchetBranch) -> Result<&'caller Key, RatchetError<'caller>> {
        if branch.len() < tree.height() {
            return Err(RatchetError{
                reason: "Branch & Tree height mismatch: Committing branch would result in desynced state",
                index: branch.root,
                height: branch.len()
            });
        }

        let mut iter: core::slice::Iter<Key> = branch.iter();
        let mut height: usize = 0;
        let mut index: usize = branch.root;

        if tree.orphans.get(0) == Some(&index) {
            tree.orphans.remove(0); // remove for BumpVec shifts elements to the left
        }

        // If we're committing a deletion, push the index to orphaned indexes for re-use later
        if branch.nodes.get(0) == tree.tombstone.as_ref() {
            tree.orphans.push(index);
        }

        while let Some(key) = iter.next() {
            tree.ensure_layer_present(height, tree.memory.get_ref(MEMORY_TREE_START_INDEX + height));
            let layer: &mut BumpVec<Key> = &mut tree.nodes[height];

            // Lol Vec.insert shifts elements to the right and there's no nice way to allocate manually
            if index >= layer.len() {
                layer.insert(index, *key);
            } else {
                layer[index] = *key;
            }

            height += 1;
            index = get_next_index(index);
        }

        return Ok(&tree.nodes[height - 1][index]);
    }

    // Do not immediately commit the key, return a commit view so we can commit on txn confirmation
    pub fn insert<'caller>(tree: &Self, key: &Key, scratch: &'caller AllocatorCell) -> Result<RatchetBranch<'caller>, RatchetError<'caller>> {
        return RatchetTree::ratchet(tree, tree.get_next_index(), key, &scratch);
    }

    pub fn remove<'caller>(tree: &Self, index: usize, scratch: &'caller AllocatorCell) -> Result<RatchetBranch<'caller>, RatchetError<'caller>> {
        // Lifetimes and mutable borrows, seriously it's pathetic, I can't do any graceful programming in this abhorrant language
        // please if anyone knows a more graceful way around this fucking shit let me know, I'm pulling my hair out. Just ugly
        // and redundant code, line after line due to mutability. Sigh.
        //tree.set(0, index, Key::default())?;
        let leaf_node_len: usize = tree.get_layer_len(0);
        let sibling_index: usize = get_sibling_index(index);
        if index >= leaf_node_len || sibling_index >= leaf_node_len {
            return Err(RatchetError{
                reason: "index provided larger than leaf-node array len",
                index: index,
                height: 0
            });
        }

        /*if tree.get(0, index) == tree.tombstone.as_ref()  {
            return Err(RatchetError{
                reason: "Key at index has already been removed",
                index: index,
                height: 0
            });
        }*/

        return RatchetTree::ratchet(tree, index, tree.tombstone.as_ref().unwrap(), scratch);
    }

    pub fn get(&self, height: usize, index: usize) -> Option<&Key> {
        if let Some(layer) = self.nodes.get(height) {
            return layer.get(index);
        }

        return None;
    }

    pub fn set(&mut self, height: usize, index: usize, value: Key) -> Result<(), RatchetError> {
        if let Some(layer) = self.nodes.get_mut(height) {
            if index >= layer.len() {
                return Err(RatchetError{
                    reason: "index provided larger than leaf-node array len",
                    index: index,
                    height: 0
                });
            }

            layer[index] = value;

            return Ok(());
        }

        return Err(RatchetError{
            reason: "No index found at specified height",
            index: index,
            height: height
        });
    }

    pub fn get_layer(&self, height: usize) -> Option<&BumpVec<Key>> {
        return self.nodes.get(height);
    }
    
    pub fn get_layer_len(&self, height: usize) -> usize {
        if let Some(layer) = self.nodes.get(height) {
            return layer.len();
        }

        return 0;
    }

    pub fn memory(&self) -> &AllocatorPool {
        return self.memory;
    }
}