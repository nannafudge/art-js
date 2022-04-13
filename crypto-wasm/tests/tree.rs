/*#![cfg(test)]
#[macro_use]
extern crate crypto_art;

use wasm_bindgen_test::*;

use crypto_art::log::*;

use crypto_art::{
    ecdh::{
        Key,
        Secret
    },
    tree::{
        RatchetBranch,
        RatchetTree,
        TreeFactory,
        SynchronousTreeFactory,
        SynchronousRatchetTree,
    }
};

use rand_core::OsRng;

#[wasm_bindgen_test]
fn test_tree_create_single() {
    let mut tree: SynchronousRatchetTree = SynchronousTreeFactory::new();
    let key: Key = Secret::random(&mut OsRng).into();

    let mut a: Vec<Key> = Vec::with_capacity(1);
    a.insert(0, key);

    info!("{:?}", a.get(0).unwrap());

    match a.get_mut(0) {
        Some(mut _k) => {
            let b: Key = Secret::random(&mut OsRng).into();
            _k.set_sk(b.sk).expect("Unable to update Secret Key");
        },
        None => {}
    }

    info!("{:?}", a.get(0).unwrap());

    //tree.insert(key).expect("Unable to insert Key into Tree.");
}

#[wasm_bindgen_test]
fn test_tree_create_pair() {
    //let tree: RatchetTree = RatchetTree::new();

    //tree.insert();
}*/