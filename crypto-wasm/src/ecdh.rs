extern crate alloc;
extern crate subtle;

use core::{
    borrow::Borrow,
    cmp::{
        PartialEq
    },
    clone::Clone,
    convert::{
        Into,
        From
    },
    marker::Copy,
    mem,
    fmt::{
        Debug,
        Formatter
    }
};

use rand_core::{
    CryptoRng,
    RngCore
};

use k256::{
    NonZeroScalar,
    PublicKey,
    FieldBytes,
    ProjectivePoint,
    AffinePoint,
    Secp256k1
};

use elliptic_curve::{
    AffineXCoordinate,
    Scalar
};

use crate::errors::ECError;

const DEFAULT_SECRET: Option<Secret> = None;
const DEFAULT_PK: AffinePoint = AffinePoint::IDENTITY;

pub fn diffie_hellman<'a>(sk: impl Borrow<NonZeroScalar>, pk: impl Borrow<AffinePoint>) -> Result<Secret, ECError<'a>> {
    let public_point = ProjectivePoint::from(*pk.borrow());
    let secret_point = (public_point * sk.borrow().as_ref()).to_affine().x();
    return Secret::from_repr(&secret_point)
}

pub trait KeyOps {
    fn diffie_hellman<'a>(&self, target: &PublicKey) -> Result<Secret, ECError<'a>>;
}

#[derive(Copy, Clone)]
pub struct Secret {
    scalar: NonZeroScalar
}

impl Secret {
    pub fn random(rng: impl CryptoRng + RngCore) -> Self {
        Self {
            scalar: NonZeroScalar::random(rng),
        }
    }

    pub fn from_repr<'a>(repr: &FieldBytes) -> Result<Self, ECError<'a>> {
        let _scalar = NonZeroScalar::from_repr(*repr);

        if _scalar.is_some().into() {
            return Ok(Self{scalar: _scalar.unwrap()});
        }

        return Err(ECError{reason: "Invalid scalar provided!"});
    }

    pub fn replace_scalar<'a>(&mut self, scalar: NonZeroScalar) -> Result<(), ECError<'a>> {
        self.scalar = scalar;

        return Ok(());
    }

    pub fn public_key(&self) -> PublicKey {
        return PublicKey::from_secret_scalar(&self.scalar);
    }
}

impl Debug for Secret {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "Key [{:?}]", self.public_key())
    }
}

impl KeyOps for Secret {
    fn diffie_hellman<'a>(&self, target: &PublicKey) -> Result<Secret, ECError<'a>> {
        return diffie_hellman(self.scalar, target.as_affine());
    }
}

impl KeyOps for PublicKey {
    fn diffie_hellman<'a>(&self, _target: &PublicKey) -> Result<Secret, ECError<'a>> {
        return Err(ECError{
            reason: "Diffie-Hellman using pubkey not allowed: Please call diffie_hellman from a Secret instead"
        });
    }
}

#[derive(Copy, Clone)]
pub struct Key {
    pub sk: Option<Secret>,
    pub pk: PublicKey
}

impl PartialEq for Key {
    fn eq(&self, other: &Key) -> bool {
        return self.pk == other.pk;
    }
}

impl Default for Key {
    fn default() -> Self {
        Self {
            sk: None,
            pk: PublicKey::from_secret_scalar(&NonZeroScalar::new(Scalar::<Secp256k1>::ONE).unwrap())
        }
    }
}

pub trait Take<T> {
    fn take(&mut self) -> T;
}

impl<'a> Take<Result<Key, ECError<'a>>> for Key {
    fn take(&mut self) -> Result<Key, ECError<'a>> {
        let new: Key = mem::take::<Key>(self);
        if !new.sk.is_some() && new.pk.as_affine() == &DEFAULT_PK {
            return Err(ECError{
                reason: &format_args!("Unable to take reference from Key {:?}", self).as_str().unwrap()
            });
        }

        return Ok(new);
    }
}

impl<'a> Key {
    pub fn new(pk: PublicKey, sk: Option<Secret>) -> Self {
        return Self {
            pk: pk,
            sk: sk
        }
    }

    pub fn set_sk(&mut self, sk: Option<Secret>) -> Result<(), ECError<'a>> {
        self.sk = sk;

        if self.sk.is_some() {
            self.pk = self.sk.unwrap().public_key()
        }

        return Ok(());
    }

    pub fn set_pk(&mut self, pk: PublicKey) -> Result<(), ECError<'a>> {
        self.pk = pk;

        match self.sk {
            Some(secret) => {
                if secret.public_key() != self.pk {
                    return Err(ECError{
                        reason: &format_args!("Invalid public key set for secret key {:?}", secret).as_str().unwrap()
                    })
                }
            },
            _ => {}
        }

        return Ok(());
    }

    pub fn set_secret_scalar(&self, scalar: NonZeroScalar) -> Result<(), ECError<'a>> {
        match self.sk {
            Some(mut secret) => {
                return secret.replace_scalar(scalar);
            },
            None => {
                return Err(ECError{
                    reason: &format_args!("No Secret Key found for key {:?}", self.pk).as_str().unwrap()
                });
            }
        }
    }

    pub fn diffie_hellman(&self, target: &Key) -> Result<Key, ECError<'a>> {
        match self.sk {
            Some(secret) => {
                match secret.diffie_hellman(&target.pk) {
                    Ok(result) => return Ok(result.into()),
                    Err(e) => return Err(e)
                }
            }
            None => {
                // Only commit to DH if the target key has a secret
                if target.sk.is_some() {
                    return target.diffie_hellman(self);
                }

                Err(ECError{reason: "No Secret Key available for Key {:?} to perform Diffie-Hellman!"})
            }
        }
    }
}

impl Debug for Key {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "Key [{:?}]", self.pk)
    }
}

impl From<Secret> for Key {
    fn from(sk: Secret) -> Key {
        return Key::new(sk.public_key(), Some(sk));
    }
}

impl From<PublicKey> for Key {
    fn from(pk: PublicKey) -> Key {
        return Key::new(pk, None);
    }
}

impl Into<PublicKey> for Key {
    fn into(self) -> PublicKey {
        return self.pk;
    }
}

impl Into<Option<Secret>> for Key {
    fn into(self) -> Option<Secret> {
        return self.sk;
    }
}