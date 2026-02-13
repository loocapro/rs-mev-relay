use super::Error;
use serde::{Deserialize, Serialize};
use serde_utils::hex::encode as hex_encode;
use std::fmt;
use std::marker::PhantomData;

use super::{
    public_key::{GenericPublicKey, TPublicKey},
    signature::{GenericSignature, TSignature},
};
use crate::blst::signature::DST;
use blst::min_pk::{PublicKey, SecretKey, Signature};
use rand::Rng;

use tree_hash::Hash256;

/// The byte-length of a BLS secret key.
pub const SECRET_KEY_BYTES_LEN: usize = 32;

/// Implemented on some struct from a BLS library so it may be used as the `point` in a
/// `GenericSecretKey`.
pub trait TSecretKey<SignaturePoint, PublicKeyPoint>: Sized {
    /// Instantiate `Self` from some secure source of entropy.
    fn random() -> Self;

    /// Signs `msg`.
    fn sign(&self, msg: Hash256) -> SignaturePoint;

    /// Returns the public key that corresponds to self.
    fn public_key(&self) -> PublicKeyPoint;

    /// Deserialize `self` from compressed bytes.
    fn deserialize(bytes: &[u8]) -> Result<Self, Error>;

    /// Serialize `self` as compressed bytes.
    fn serialize(&self) -> [u8; SECRET_KEY_BYTES_LEN];
}

impl TSecretKey<Signature, PublicKey> for SecretKey {
    fn random() -> Self {
        let rng = &mut rand::thread_rng();
        let ikm: [u8; 32] = rng.gen();

        Self::key_gen(&ikm, &[]).unwrap()
    }

    fn public_key(&self) -> PublicKey {
        self.sk_to_pk()
    }

    fn sign(&self, msg: Hash256) -> Signature {
        self.sign(msg.as_bytes(), DST, &[])
    }
    fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
        Self::from_bytes(bytes).map_err(Into::into)
    }

    /// Serialize `self` as compressed bytes.
    ///
    /// ## Note
    ///
    /// The bytes that are returned are the unencrypted secret key. This is sensitive cryptographic
    /// material.
    fn serialize(&self) -> [u8; SECRET_KEY_BYTES_LEN] {
        self.to_bytes()
    }
}

#[derive(Clone, Debug)]
/// A BLS secret key that is generic across some BLS point (`Sec`).
pub struct GenericSecretKey<Sig, Pub, Sec> {
    /// The underlying point which performs *actual* cryptographic operations.
    point: Sec,
    _phantom_signature: PhantomData<Sig>,
    _phantom_public_key: PhantomData<Pub>,
}

impl<Sig, Pub, Sec> GenericSecretKey<Sig, Pub, Sec>
where
    Sig: TSignature<Pub>,
    Pub: TPublicKey,
    Sec: TSecretKey<Sig, Pub>,
{
    /// Instantiate `Self` from some secure source of entropy.
    pub fn random() -> Self {
        Self {
            point: Sec::random(),
            _phantom_signature: PhantomData,
            _phantom_public_key: PhantomData,
        }
    }

    /// Signs `msg`.
    pub fn sign(&self, msg: Hash256) -> GenericSignature<Pub, Sig> {
        let is_infinity = false;
        GenericSignature::from_point(self.point.sign(msg), is_infinity)
    }

    /// Returns the public key that corresponds to self.
    pub fn public_key(&self) -> GenericPublicKey<Pub> {
        GenericPublicKey::from_point(self.point.public_key())
    }

    /// Deserialize `self` from compressed bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() != SECRET_KEY_BYTES_LEN {
            Err(Error::InvalidSecretKeyLength {
                got: bytes.len(),
                expected: SECRET_KEY_BYTES_LEN,
            })
        } else if bytes.iter().all(|b| *b == 0) {
            Err(Error::InvalidZeroSecretKey)
        } else {
            Ok(Self {
                point: Sec::deserialize(bytes)?,
                _phantom_signature: PhantomData,
                _phantom_public_key: PhantomData,
            })
        }
    }
    /// Serialize `self` as compressed bytes.
    ///
    /// ## Note
    ///
    /// The bytes that are returned are the unencrypted secret key. This is sensitive cryptographic
    /// material.
    pub fn serialize(&self) -> [u8; SECRET_KEY_BYTES_LEN] {
        self.point.serialize()
    }
}

/// BLS secret key that uses the `blst` library.
pub type BlsSecretKey = GenericSecretKey<Signature, PublicKey, SecretKey>;

impl<'de, T: TSecretKey<Signature, PublicKey>> Deserialize<'de>
    for GenericSecretKey<Signature, PublicKey, T>
{
    impl_serde_deserialize!();
}

impl<T: TSecretKey<Signature, PublicKey>> std::str::FromStr
    for GenericSecretKey<Signature, PublicKey, T>
{
    impl_from_str!();
}

impl<T: TSecretKey<Signature, PublicKey>> fmt::Display
    for GenericSecretKey<Signature, PublicKey, T>
{
    impl_display!();
}

impl<T: TSecretKey<Signature, PublicKey>> Serialize for GenericSecretKey<Signature, PublicKey, T> {
    impl_serde_serialize!();
}
