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
    tree::RatchetError,
    tree::RatchetErrorCause
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
    let memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 4, 32);
    let tree: RatchetTree = RatchetTree::new(&memory);

    let key: Key = Secret::random(&mut OsRng).into();

    let scratch: AllocatorCell = memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let res: RatchetBranch = tree.insert(&key, &scratch).expect("Error inserting key into tree");
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
    let memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 8, 32);
    let mut tree: RatchetTree = RatchetTree::new(&memory);

    let key_one: Key = Secret::random(&mut OsRng).into();
    let key_two: Key = Secret::random(&mut OsRng).into();
    
    let scratch: AllocatorCell = memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let branch_one: RatchetBranch = tree.insert(&key_one, &scratch).expect("Error inserting key_one into tree");

    assert_eq!(branch_one.len(), 1);
    assert_eq!(branch_one.get_node(0), Some(&key_one));

    let result_one: &Key = tree.commit(&branch_one, &memory).expect("Unable to commit branch_one to tree");

    // Only key in the tree
    assert_eq!(result_one, &key_one);

    assert_eq!(tree.get_next_index(), 2);
    assert_eq!(tree.height(), 0);

    let branch_two: RatchetBranch = tree.insert(&key_two, &scratch).expect("Error inserting key_one into tree");

    assert_eq!(branch_two.len(), 2);
    assert_eq!(branch_two.get_node(0), Some(&key_two));
    assert_ne!(branch_two.get_node(1), Some(&key_one));
    assert_ne!(branch_two.get_node(1), Some(&key_two));

    let expected_dh_result: &Key = &branch_two.get_node(1).unwrap();
    let result_two: &Key = tree.commit(&branch_two, &memory).expect("Unable to commit branch_two to tree");

    // Resulting key is the diffie-hellman result between the two keys
    assert_eq!(result_two, expected_dh_result);

    assert_eq!(tree.get_next_index(), 3);
    assert_eq!(tree.height(), 1);
}

#[wasm_bindgen_test]
fn test_tree_insert_multiple() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(12);
    let test_allocator: Bump = AllocatorPool::create_bumpalo::<Key>(32);

    let tree_one_memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 12, 32);
    let mut tree_one: RatchetTree = RatchetTree::new(&tree_one_memory);

    let mut keys: Vec<Key> = Vec::new_in(&test_allocator);

    for i in 1..33 {
        let key: Key = Secret::random(&mut OsRng).into();
        let scratch: AllocatorCell = tree_one_memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = tree_one.insert(&key, &scratch).expect("Error inserting key_one into tree_one");

        keys.push(key);

        tree_one.commit(&branch, &tree_one_memory).expect("Unable to commit branch_two to tree");

        // Tree indexes start from 1
        assert_eq!(keys.get(i - 1), tree_one.get(0, i));
    }

    assert_eq!(tree_one.get_next_index(), 33);
    assert_eq!(tree_one.height(), 5);

    let tree_two_memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 12, 32);
    let mut tree_two: RatchetTree = RatchetTree::new(&tree_two_memory);

    for key in keys {
        let scratch: AllocatorCell = tree_two_memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = tree_two.insert(&key, &scratch).expect("Error inserting key_one into tree_two");

        tree_two.commit(&branch, &tree_two_memory).expect("Unable to commit branch_two to tree");
    }

    assert_eq!(tree_one.get(tree_one.height(), 1), tree_two.get(tree_two.height(), 1));

    let mut height: usize = 32;
    for i in 0..tree_one.height() {
        assert_eq!(tree_one.get_layer(i).expect("No layer found").len() - 1, height);
        height /= 2;
    }
}

#[wasm_bindgen_test]
fn test_tree_delete_single() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(12);
    let test_allocator: Bump = AllocatorPool::create_bumpalo::<Key>(32);

    let tree_memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 12, 32);
    let mut tree: RatchetTree = RatchetTree::new(&tree_memory);

    let mut keys: Vec<Key> = Vec::new_in(&test_allocator);

    for _ in 0..16 {
        let key: Key = Secret::random(&mut OsRng).into();
        let scratch: AllocatorCell = tree_memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = tree.insert(&key, &scratch).expect("Error inserting key_one into tree_one");

        keys.push(key);

        tree.commit(&branch, &tree_memory).expect("Unable to commit add_branch to tree");
    }

    let scratch: AllocatorCell = tree_memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let remove_branch: RatchetBranch = RatchetTree::remove(&tree, 16, &scratch).expect("Unable to compute remove for tree");

    assert!(tree.commit(&remove_branch, &tree_memory).is_ok());
    assert_eq!(tree.get(0, 16), tree.tombstone.as_ref());
    assert_eq!(tree.get_next_index(), 16);

    // Tree index starts at 1, keys (Vec) starts at 0
    assert_eq!(tree.get(0, 15), keys.get(14));
}

#[wasm_bindgen_test]
fn test_tree_delete_insert_complex() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(12);
    let test_allocator: Bump = AllocatorPool::create_bumpalo::<Key>(8);

    let tree_memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 12, 32);
    let mut tree: RatchetTree = RatchetTree::new(&tree_memory);

    let mut keys: Vec<Key> = Vec::new_in(&test_allocator);

    for _ in 0..7 {
        keys.push(Secret::random(&mut OsRng).into());
    }

    /*
     *       (ABCD, EFG)
     *        /       \
     *     (ABCD)    (EFG)
     *     /    \     /  \
     *   (AB)  (CD)  (EF) \
     *   /  \  /  \  /  \  \
     *   A  B  C  D  E  F   G
    */

    let ab: Key = keys[0].diffie_hellman(&keys[1]).expect("AB Diffie-Hellman failed");
    let cd: Key = keys[2].diffie_hellman(&keys[3]).expect("CD Diffie-Hellman failed");
    let ef: Key = keys[4].diffie_hellman(&keys[5]).expect("EF Diffie-Hellman failed");

    let abcd: Key = ab.diffie_hellman(&cd).expect("ABCD Diffie-Hellman failed");
    let efg: Key = ef.diffie_hellman(&keys[6]).expect("EFG Diffie-Hellman failed");

    let abcdefg: Key = abcd.diffie_hellman(&efg).expect("ABCDEFG Diffie-Hellman failed");

    for key in keys.clone() {
        let scratch: AllocatorCell = tree_memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = tree.insert(&key, &scratch).expect("Error inserting key into tree");

        tree.commit(&branch, &tree_memory).expect("Unable to commit branch to tree");
    }

    assert_eq!(&abcdefg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *       (ABC, EFG)
     *        /       \
     *     (ABC)     (EFG)
     *     /    \     /  \
     *   (AB)   (C)  (EF) \
     *   /  \  /  \  /  \  \
     *   A  B  C  X  E  F   G
    */

    let abc = ab.diffie_hellman(&keys[2]).expect("ABCX Diffie-Hellman failed");
    let abcefg = abc.diffie_hellman(&efg).expect("ABCXEFG Diffie-Hellman failed");

    // Remove D from tree
    let scratch: AllocatorCell = tree_memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let remove_branch_d: RatchetBranch = RatchetTree::remove(&tree, 4, &scratch).expect("Unable to compute remove for tree");

    tree.commit(&remove_branch_d, &tree_memory).expect("Unable to commit remove_branch_d to tree");

    assert_eq!(tree.get(0, 4), tree.tombstone.as_ref());
    assert_eq!(&abcefg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *        (AB, EFG)
     *        /       \
     *     (AB)      (EFG)
     *     /   \      /  \
     *   (AB)  (X)   (EF) \
     *   /  \  /  \  /  \  \
     *   A  B  X  X  E  F   G
    */

    let remove_branch_c: RatchetBranch = RatchetTree::remove(&tree, 3, &scratch).expect("Unable to compute remove for tree");
    tree.commit(&remove_branch_c, &tree_memory).expect("Unable to commit remove_branch_c to tree");

    let abefg = ab.diffie_hellman(&efg).expect("ABXXEFG Diffie-Hellman failed");
    assert_eq!(&abefg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *        (B, EFG)
     *        /       \
     *      (B)      (EFG)
     *     /   \      /  \
     *   (B)   (X)   (EF) \
     *   /  \  /  \  /  \  \
     *   X  B  X  X  E  F   G
    */

    let remove_branch_a: RatchetBranch = RatchetTree::remove(&tree, 1, &scratch).expect("Unable to compute remove for tree");
    tree.commit(&remove_branch_a, &tree_memory).expect("Unable to commit remove_branch_a to tree");

    let befg = keys[1].diffie_hellman(&efg).expect("BEFG Diffie-Hellman failed");
    assert_eq!(&befg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *          (EFG)
     *        /       \
     *      (X)      (EFG)
     *     /   \      /  \
     *   (X)  (X)    (EF) \
     *   /  \  /  \  /  \  \
     *   X  X  X  X  E  F   G
    */

    let remove_branch_b: RatchetBranch = RatchetTree::remove(&tree, 2, &scratch).expect("Unable to compute remove for tree");
    tree.commit(&remove_branch_b, &tree_memory).expect("Unable to commit remove_branch_b to tree");

    assert_eq!(&efg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *        (B, EFG)
     *        /       \
     *      (B)      (EFG)
     *     /   \      /  \
     *   (B)   (X)   (EF) \
     *   /  \  /  \  /  \  \
     *   X  B  X  X  E  F   G
    */

    let add_branch_b: RatchetBranch = tree.insert(&keys[1], &scratch).expect("Unable to compute insert for tree");
    tree.commit(&add_branch_b, &tree_memory).expect("Unable to commit add_branch_b to tree");

    assert_eq!(&befg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *        (AB, EFG)
     *        /       \
     *     (AB)      (EFG)
     *     /   \      /  \
     *   (AB)  (X)   (EF) \
     *   /  \  /  \  /  \  \
     *   A  B  X  X  E  F   G
    */

    let add_branch_a: RatchetBranch = tree.insert(&keys[0], &scratch).expect("Unable to compute insert for tree");
    tree.commit(&add_branch_a, &tree_memory).expect("Unable to commit add_branch_b to tree");

    assert_eq!(&abefg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *       (ABC, EFG)
     *        /       \
     *     (ABC)     (EFG)
     *     /    \     /  \
     *   (AB)   (C)  (EF) \
     *   /  \  /  \  /  \  \
     *   A  B  C  X  E  F   G
    */

    let add_branch_c: RatchetBranch = tree.insert(&keys[2], &scratch).expect("Unable to compute insert for tree");
    tree.commit(&add_branch_c, &tree_memory).expect("Unable to commit add_branch_c to tree");

    assert_eq!(&abcefg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *       (ABCD, EFG)
     *        /       \
     *     (ABCD)    (EFG)
     *     /    \     /  \
     *   (AB)  (CD)  (EF) \
     *   /  \  /  \  /  \  \
     *   A  B  C  D  E  F   G
    */

    let add_branch_d: RatchetBranch = tree.insert(&keys[3], &scratch).expect("Unable to compute insert for tree");
    tree.commit(&add_branch_d, &tree_memory).expect("Unable to commit add_branch_c to tree");

    assert_eq!(&abcdefg, tree.get(tree.height(), 1).expect("Could not get final result from tree"));

    /*
     *        (ABCD, EFGH)
     *        /          \
     *     (ABCD)      (EFGH)
     *     /    \     /     \
     *   (AB)  (CD)  (EF)  (GH)
     *   /  \  /  \  /  \  /  \
     *   A  B  C  D  E  F  G  H
    */

    let h: Key = Secret::random(&mut OsRng).into();
    let gh: Key = keys[6].diffie_hellman(&h).expect("GH Diffie-Hellman failed");
    let efgh: Key = ef.diffie_hellman(&gh).expect("EFGH Diffie-Hellman failed");
    let abcdefgh: Key = abcd.diffie_hellman(&efgh).expect("ABCDEFGH Diffie-Hellman failed");

    let add_branch_h: RatchetBranch = tree.insert(&h, &scratch).expect("Unable to compute insert for tree");
    tree.commit(&add_branch_h, &tree_memory).expect("Unable to commit add_branch_h to tree");

    assert_eq!(&abcdefgh, tree.get(tree.height(), 1).expect("Could not get final result from tree"));
}

#[wasm_bindgen_test]
fn test_tree_commit_oom_workflow() {
    let root_allocator: Bump = AllocatorPool::create_bumpalo::<&Bump>(4);

    let mut memory: AllocatorPool = AllocatorPool::new_with_init::<Key>(&root_allocator, 6, 16);
    let mut tree: RatchetTree = RatchetTree::new(&memory);

    let key: Key = Secret::random(&mut OsRng).into();

    for _ in 0..4 {
        let scratch: AllocatorCell = memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
        let branch: RatchetBranch = tree.insert(&key, &scratch).expect("Error inserting key_one into tree");

        tree.commit(&branch, &memory).expect("Unable to commit branch to tree");
    }

    let scratch: AllocatorCell = memory.get(crypto_art::tree::MEMORY_BRANCH_INDEX);
    let branch: RatchetBranch = tree.insert(&key, &scratch).expect("Error inserting key_one into tree");

    let error: RatchetError = tree.commit(&branch, &memory).expect_err("No OOM Error found, problemo");
    assert_eq!(error.cause, RatchetErrorCause::OOM.into());
}