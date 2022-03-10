extern crate subtle;

use k256::{
    NonZeroScalar,
    PublicKey,
    FieldBytes,
    ecdh::SharedSecret
};

use elliptic_curve::{
    ecdh::diffie_hellman
};

use crate::errors::ECError;

pub struct DerivedSecret {
    scalar: NonZeroScalar
}

impl DerivedSecret {
    pub fn from_repr(repr: &FieldBytes) -> Result<Self, ECError> {
        let _scalar = NonZeroScalar::from_repr(*repr);

        if _scalar.is_some().into() {
            return Ok(Self{scalar: _scalar.unwrap()});
        }

        return Err(ECError{reason: "Invalid scalar provided!"});
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from_secret_scalar(&self.scalar)
    }

    pub fn diffie_hellman(&self, public_key: &PublicKey) -> SharedSecret {
        diffie_hellman(&self.scalar, public_key.as_affine())
    }
}