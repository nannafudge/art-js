#![cfg(test)]
#[macro_use]
extern crate elliptic_curve;
extern crate crypto_art;
extern crate alloc;

use core::convert;

use k256::Secp256k1;
use k256::PublicKey;
use elliptic_curve::ScalarCore;

use crypto_art::ecdh::{
    Secret,
    KeyOps,
    Key,
};

use crypto_art::log::*;

use wasm_bindgen_test::*;
use rand_core::OsRng;

// This test should NEVER fail, and this scenario should NEVER happen
#[wasm_bindgen_test]
fn test_invalid_scalar_dh() {
    let scalar: ScalarCore<Secp256k1> = ScalarCore::<Secp256k1>::ZERO;
    let result = Secret::from_repr(&scalar.to_be_bytes());

    assert!(result.is_err())
}

#[wasm_bindgen_test]
fn test_public_key_err_dh() {
    let s1: Secret = Secret::random(&mut OsRng);
    let p2: PublicKey = Secret::random(&mut OsRng).public_key();

    let result = p2.diffie_hellman(&s1.public_key());
    assert!(result.is_err())
}

#[wasm_bindgen_test]
fn test_container_public_key_err_dh() {
    let p1: Key = Secret::random(&mut OsRng).public_key().into();
    let p2: Key = Secret::random(&mut OsRng).public_key().into();

    let result = p2.diffie_hellman(&p1);
    assert!(result.is_err())
}

#[wasm_bindgen_test]
fn test_no_container_dh() {
    let s1: Secret = Secret::random(&mut OsRng);
    let s2: Secret = Secret::random(&mut OsRng);

    let s1p2: Secret = s1.diffie_hellman(&(s2.public_key()))
        .expect("Unable to derive EC pair from secret value for s1p2!");
    let s2p1: Secret = s2.diffie_hellman(&(s1.public_key()))
        .expect("Unable to derive EC pair from secret value for s2p1!");

    // Ensure the derived keypair identities are the same
    assert_eq!(s1p2.public_key().as_affine(), s2p1.public_key().as_affine())
}

#[wasm_bindgen_test]
fn test_key_container_dh() {
    let s1: Key = Secret::random(&mut OsRng).into();
    let s2: Key = Secret::random(&mut OsRng).into();

    let s1p2: Key = s1.diffie_hellman(&s2)
        .expect("Unable to derive EC keypair from key containers");
    let s2p1: Key = s2.diffie_hellman(&s1)
        .expect("Unable to derive EC keypair from key containers");

    assert_eq!(s1p2.pk, s2p1.pk)
}