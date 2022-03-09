#![cfg(test)]
#[macro_use]
extern crate crypto_art;

use wasm_bindgen_test::*;
use crypto_art::log::*;

#[wasm_bindgen_test]
fn test_info() {
    info!("Hello, World!");
    info!("{:?} Formatter", "Hello, World!");
}

#[wasm_bindgen_test]
fn test_error() {
    error!("Error!");
    error!("{:?} Formatter", "Error!");
}