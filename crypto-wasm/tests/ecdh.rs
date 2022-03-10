#![cfg(test)]
#[macro_use]
extern crate crypto_art;

use k256::{
    EncodedPoint,
    PublicKey
};

use k256::ecdh::{
    EphemeralSecret,
    SharedSecret
};

use crypto_art::ecdh::DerivedSecret;

use elliptic_curve::rand_core::OsRng;
use wasm_bindgen_test::*;
use crypto_art::log::*;

#[wasm_bindgen_test]
fn test_derive_ecdh_keypair() {
    let s1: EphemeralSecret = EphemeralSecret::random(&mut OsRng);
    let s2: EphemeralSecret = EphemeralSecret::random(&mut OsRng);

    // Test theoretical deserialization and handshake
    let p1_bytes: EncodedPoint = EncodedPoint::from(s1.public_key());
    // s2 decodes p1's public key from compressed point
    let p1: PublicKey = PublicKey::from_sec1_bytes(p1_bytes.as_ref())
        .expect("Unable to deserialize Public Key!");
    
    let p2_bytes: EncodedPoint = EncodedPoint::from(s2.public_key());
    // s1 decodes p2's public key from compressed point
    let p2: PublicKey = PublicKey::from_sec1_bytes(p2_bytes.as_ref())
        .expect("Unable to deserialize Public Key!");

    let s1p2: SharedSecret = s1.diffie_hellman(&p2);
    let s2p1: SharedSecret = s2.diffie_hellman(&p1);

    let s1p2keypair = DerivedSecret::from_repr(s1p2.as_bytes())
        .expect("Unable to derive EC pair from secret value for s1p2!");

    let s2p1keypair = DerivedSecret::from_repr(s1p2.as_bytes())
        .expect("Unable to derive EC pair from secret value for s2p1!");
    
    // Ensure the derived keypair identities are the same
    assert_eq!(s1p2keypair.public_key().as_affine(), s2p1keypair.public_key().as_affine())
}