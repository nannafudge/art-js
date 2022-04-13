extern crate alloc;

use async_trait::async_trait;
use alloc::{
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
        Context
    },
    future::{
        Future
    },
    pin::Pin,
    ops::Deref
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

pub trait New {
    fn new() -> Self;
}

#[async_trait]
pub trait TreeOperations {
    async fn search(&self);
    async fn ratchet(&mut self, index: usize) -> Result<&Key, RatchetError>;
    async fn insert(&mut self, key: &Key) -> Result<&Key, RatchetError>;
}

// TODO: Implement Clone/Copy for tree cache
//#[derive(Debug)]
pub struct RatchetTree {
    nodes: Vec<Vec<Key>>,
    orphans: HashSet<usize>
}

pub struct RatchetBranch {
    nodes: HashMap<usize, Key>
}

pub struct RatchetIter {
    index: usize,
    curr_index: usize,
    height: usize,
    curr_height: usize
}

impl RatchetIter {
    pub fn new(index: usize, height: usize, curr_height: usize) -> Self {
        return Self {
            index: index,
            curr_index: index,
            height: height,
            curr_height: curr_height
        };
    }

    pub fn reset(&mut self) {
        self.curr_index = self.index;
    }
}

impl RatchetBranch {
    fn new() -> Self {
        return Self {
            nodes: HashMap::new()
        }
    }

    fn add_node(&mut self, index: usize, key: Key) {
        self.nodes.insert(index, key);
    }

    fn get_node(&self, index: &usize) -> Option<&Key> {
        return self.nodes.get(index);
    }

    fn iter(&self) -> hash_map::Iter<usize, Key> {
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
        * Secondly, it ensures the iterator properly exists once we no longer have at least (2) key pairs to DH with
        */
        if self.curr_index <= 1 && self.curr_height >= self.height {
            return None;
        }

        let next: Self::Item = (self.curr_height, self.index, get_sibling_index(self.index));

        self.curr_index = get_next_index(self.index);
        self.curr_height += 1;

        return Some(next);
    }
}

/*
* This data structure perhaps makes some naiive assumptions: We assume that it is not possible to remove
* the first index (0) in the nodes array. TODO: Make it impossible to do such with a wrapper object around Vec?
*/
impl RatchetTree {
    pub fn new() -> Self {
        let mut nodes: Vec<Vec<Key>> = Vec::with_capacity(1);
        nodes.insert(0, Vec::with_capacity(1));

        return Self {
            nodes: nodes,
            orphans: HashSet::new()
        };
    }

    pub fn get_next_index(&self) -> usize {
        if self.orphans.len() > 0 {
            return *self.orphans.iter().next().unwrap();
        } else {
            return self.nodes[0].len() + 1;
        }
    }

    pub fn height(&self) -> usize {
        return (self.nodes[0].len() as f64).log(2.0).ceil() as usize - 1;
    }

    pub fn iter(&self, index: usize) -> RatchetIter {
        return RatchetIter::new(index, self.height(), 0);
    }

    pub fn ratchet(&mut self, index: usize, key: &Key) -> Result<RatchetBranch, RatchetError> {
        let mut iterator: RatchetIter = self.iter(index);
        let mut branch: RatchetBranch = RatchetBranch::new();

        // Root of the branch is our node
        branch.add_node(index, *key);

        // Two phase commit: parity check to ensure values are present before committing
        while let Some(key_tuple) = iterator.next() {
            //if 
        }

        /*match self.nodes[0].get_mut(index) {
            Some(mut _key) => {
                _key.set_sk(key.sk);
            },
            None => {
                return Err(
                    RatchetError{
                        reason: "No key exists at specified index",
                        index: index,
                        height: 0
                });
            }
        }*/

        while let Some(key_tuple) = iterator.next() {
            let height: usize = key_tuple.0;
            let k1: Option<&Key> = self.nodes[height].get(key_tuple.1); // Key 1
            let k2: Option<&Key> = self.nodes[height].get(key_tuple.2); // Key 2

            /*match k1.diffie_hellman(k2) {
                Ok(result) => {
                    let result_index: usize = get_next_index(key_tuple.0);
                    branch.add_node(result_index, result);
                },
                Err(e) => {
                    return Err(RatchetError{
                        reason: &format_args!("Unable to Ratchet tree: {:?}", e.reason).as_str().unwrap(),
                        index: key_tuple.1,
                        height: height
                    });
                }
            }*/
        }

        return Ok(branch);
    }

    pub fn commit(&mut self, branch: &RatchetBranch) -> Result<&Key, RatchetError> {
        if branch.len() < self.height() - 1 { // Omit top most node in height, as it is the Tree Secret
            return Err(RatchetError{
                reason: "Branch & Tree height mismatch: Committing branch would result in desynced state",
                index: 0,
                height: branch.len()
            });
        }

        let mut iter: hash_map::Iter<usize, Key> = branch.iter();
        let mut height: usize = 0;

        while let Some(node) = iter.next() {
            if self.nodes.get(height).is_none() {
                self.nodes.insert(height, Vec::with_capacity(1)); 
            }

            self.nodes[height][*node.0] = *node.1;
            height += 1;
        }

        return Ok(&self.nodes[self.height()][1]);
    }

    // Do not immediately commit the key, return a commit view so we can commit on txn confirmation
    pub fn insert(&mut self, key: Key) -> Result<RatchetBranch, RatchetError> {
        return self.ratchet(self.get_next_index(), &key);
    }

    pub fn rebalance() {
        return;
    }
}