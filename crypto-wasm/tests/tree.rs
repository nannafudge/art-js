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
    let tree: RatchetTree = RatchetTree::new(&mut memory);

    assert_eq!(tree.get_next_index(), 1);
    assert_eq!(tree.height(), 0);
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
    assert_eq!(res.get_node(0), Some(&Some(key)));

    drop(res);

    // Should not change the height or next index, we commit the branch later, once the
    // consensus algorithm has given us a supermajority on our message
    assert_eq!(tree.get_next_index(), 1);
    assert_eq!(tree.height(), 0);
}

#[wasm_bindgen_test]
fn test_tree_insert_double() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);
    let mut memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 8, 32);
    let mut tree: RatchetTree = RatchetTree::new(&mut memory);

    let key_one: Key = Secret::random(&mut OsRng).into();
    let key_two: Key = Secret::random(&mut OsRng).into();
    
    let scratch: AllocatorCell = tree.memory().get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let branch_one: RatchetBranch = RatchetTree::insert(&tree, &key_one, &scratch).expect("Error inserting key_one into tree");

    assert_eq!(branch_one.len(), 1);
    assert_eq!(branch_one.get_node(0), Some(&Some(key_one)));

    let result_one: &Key = RatchetTree::commit(&mut tree, &branch_one).expect("Unable to commit branch_one to tree");

    // Only key in the tree
    assert_eq!(result_one, &key_one);

    assert_eq!(tree.get_next_index(), 2);
    assert_eq!(tree.height(), 0);

    let branch_two: RatchetBranch = RatchetTree::insert(&tree, &key_two, &scratch).expect("Error inserting key_one into tree");

    assert_eq!(branch_two.len(), 2);
    assert_eq!(branch_two.get_node(0), Some(&Some(key_two)));
    assert_ne!(branch_two.get_node(1), Some(&Some(key_one)));
    assert_ne!(branch_two.get_node(1), Some(&Some(key_two)));

    let expected_dh_result: &Key = &branch_two.get_node(1).unwrap().unwrap();
    let result_two: &Key = RatchetTree::commit(&mut tree, &branch_two).expect("Unable to commit branch_two to tree");

    // Resulting key is the diffie-hellman result between the two keys
    assert_eq!(result_two, expected_dh_result);

    assert_eq!(tree.get_next_index(), 3);
    assert_eq!(tree.height(), 1);
}

#[wasm_bindgen_test]
fn test_tree_insert_multiple() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(12);
    let test_allocator: Bump = AllocatorPool::create_bumpalo::<Key>(32);

    let mut tree_one_memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 12, 32);
    let mut tree_one: RatchetTree = RatchetTree::new(&mut tree_one_memory);

    let mut keys: Vec<Key> = Vec::new_in(&test_allocator);

    for _ in 0..32 {
        let key: Key = Secret::random(&mut OsRng).into();
        let scratch: AllocatorCell = tree_one.memory().get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = RatchetTree::insert(&tree_one, &key, &scratch).expect("Error inserting key_one into tree_one");

        keys.push(key);

        RatchetTree::commit(&mut tree_one, &branch).expect("Unable to commit branch_two to tree");
    }

    assert_eq!(tree_one.get_next_index(), 33);
    assert_eq!(tree_one.height(), 5);

    let mut tree_two_memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 12, 32);
    let mut tree_two: RatchetTree = RatchetTree::new(&mut tree_two_memory);

    for key in keys {
        let scratch: AllocatorCell = tree_two.memory().get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = RatchetTree::insert(&tree_two, &key, &scratch).expect("Error inserting key_one into tree_two");

        RatchetTree::commit(&mut tree_two, &branch).expect("Unable to commit branch_two to tree");
    }

    assert_eq!(tree_one.get(tree_one.height(), 1), tree_two.get(tree_two.height(), 1));

    let mut height: usize = 32;
    for i in 0..tree_one.height() {
        assert_eq!(tree_one.get_layer(i).expect("No layer found").len() - 1, height);
        height /= 2;
    }
}

#[wasm_bindgen_test]
fn test_tree_validity() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(12);
    let test_allocator: Bump = AllocatorPool::create_bumpalo::<Key>(8);

    let mut tree_memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 12, 32);
    let mut tree: RatchetTree = RatchetTree::new(&mut tree_memory);

    let mut keys: Vec<Key> = Vec::new_in(&test_allocator);

    for _ in 1..8 {
        keys.push(Secret::random(&mut OsRng).into());
    }

    /*
     *       (ABCD, EFG)
     *        /       \
     *     (ABCD)    (EFG)
     *     /    \     /  \
     *   (AB)  (CD)  (EF) \
     *   /  \  /  \  /  \  \
     *   A  B  C  D  E  F  G
    */
    let ab: Key = keys[0].diffie_hellman(&keys[1]).expect("AB Diffie-Hellman failed");
    let cd: Key = keys[2].diffie_hellman(&keys[3]).expect("CD Diffie-Hellman failed");
    let ef: Key = keys[4].diffie_hellman(&keys[5]).expect("EF Diffie-Hellman failed");

    let abcd: Key = ab.diffie_hellman(&cd).expect("ABCD Diffie-Hellman failed");
    let efg: Key = ef.diffie_hellman(&keys[6]).expect("EFG Diffie-Hellman failed");

    let abcdefg: Key = abcd.diffie_hellman(&efg).expect("ABCDEFG Diffie-Hellman failed");

    for key in keys {
        let scratch: AllocatorCell = tree.memory().get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = RatchetTree::insert(&tree, &key, &scratch).expect("Error inserting key into tree");

        RatchetTree::commit(&mut tree, &branch).expect("Unable to commit branch to tree");
    }

    assert_eq!(&abcdefg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));
}