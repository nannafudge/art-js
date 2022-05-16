#![cfg(test)]
#[macro_use]
extern crate crypto_art;

use wasm_bindgen_test::*;

use crypto_art::log::*;

use crypto_art::{
    ecdh::Key,
    ecdh::Secret,
    mem::AllocatorPool,
    mem::AllocatorCell,
    tree::RatchetBranch,
    tree::RatchetTree,
    tree::TreeOperations
};

use bumpalo::{
    Bump,
    collections::Vec
};

use rand_core::OsRng;

#[wasm_bindgen_test]
fn test_tree_create() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);
    let mut memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 4, 32);
    let mut tree: RatchetTree = RatchetTree::new(&mut memory);

    assert_eq!(tree.get_next_index(), 1);
    assert_eq!(tree.height(), 0);
    //tree.insert();
}

#[wasm_bindgen_test]
fn test_tree_insert_single() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);
    let mut memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 4, 32);
    let tree: RatchetTree = RatchetTree::new(&mut memory);

    let key: Key = Secret::random(&mut OsRng).into();

    let scratch: AllocatorCell = tree.memory().get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let res: RatchetBranch = RatchetTree::insert(&tree, &key, &scratch).expect("Error inserting key into tree");
    assert_eq!(res.len(), 1);
    assert_eq!(res.get_node(0), Some(&key));

    drop(res);

    // Should not change the height or next index, we commit the branch later, once the
    // consensus algorithm has given us a supermajority on our message
    assert_eq!(tree.get_next_index(), 1);
    assert_eq!(tree.height(), 0);
}

#[wasm_bindgen_test]
fn test_tree_insert_double() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);
    let mut memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 4, 32);
    let mut tree: RatchetTree = RatchetTree::new(&mut memory);

    let key: Key = Secret::random(&mut OsRng).into();

    let scratch: AllocatorCell = tree.memory().get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let branch: RatchetBranch = RatchetTree::insert(&tree, &key, &scratch).expect("Error inserting key into tree");

    assert_eq!(branch.len(), 1);
    assert_eq!(branch.get_node(0), Some(&key));

    let res = RatchetTree::commit(&mut tree, &branch);

    info!("{:?}", res.is_ok());

    //assert_eq!(tree.get_next_index(), 2);
    //assert_eq!(tree.height(), 0);
}