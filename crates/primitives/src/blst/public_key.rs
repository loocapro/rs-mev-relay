use super::secret_key::BlsSecretKey;
use super::Error;
use blst::min_pk::PublicKey;
use serde::Deserialize;
use serde::Serialize;
use serde_utils::hex::encode as hex_encode;
use ssz::Decode;
use ssz::Encode;
use std::fmt;
use tree_hash::TreeHash;

/// The byte-length of a BLS public key when serialized in compressed form.
pub const PUBLIC_KEY_BYTES_LEN: usize = 48;
/// Implemented on some struct from a BLS library so it may be used as the `point` in a
/// `GenericPublicKey`.
pub trait TPublicKey: Sized + Clone {
    /// Serialize `self` as compressed bytes.
    fn serialize(&self) -> [u8; PUBLIC_KEY_BYTES_LEN];

    /// Deserialize `self` from compressed bytes.
    fn deserialize(bytes: &[u8]) -> Result<Self, Error>;
}

impl TPublicKey for PublicKey {
    fn serialize(&self) -> [u8; PUBLIC_KEY_BYTES_LEN] {
        self.compress()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
        // key_validate accepts uncompressed bytes too so enforce byte length here.
        // It also does subgroup checks, noting infinity check is done in `generic_public_key.rs`.
        if bytes.len() != PUBLIC_KEY_BYTES_LEN {
            return Err(Error::InvalidByteLength {
                got: bytes.len(),
                expected: PUBLIC_KEY_BYTES_LEN,
            });
        }
        Self::key_validate(bytes).map_err(Into::into)
    }
}

/// Represents the public key at infinity.
pub const INFINITY_PUBLIC_KEY: [u8; PUBLIC_KEY_BYTES_LEN] = [
    0xc0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// A BLS public key that is generic across some BLS point (`Pub`).
///
/// Provides generic functionality whilst deferring all serious cryptographic operations to `Pub`.
#[derive(Clone, Copy)]
pub struct GenericPublicKey<Pub> {
    /// The underlying point which performs *actual* cryptographic operations.
    point: Pub,
}

impl<Pub> GenericPublicKey<Pub>
where
    Pub: TPublicKey,
{
    /// Instantiates `Self` from a `point`.
    pub(crate) fn from_point(point: Pub) -> Self {
        Self { point }
    }

    /// Returns a reference to the underlying BLS point.
    pub(crate) fn point(&self) -> &Pub {
        &self.point
    }

    /// Returns `self.serialize()` as a `0x`-prefixed hex string.
    pub fn as_hex_string(&self) -> String {
        format!("{:?}", self)
    }

    /// Serialize `self` as compressed bytes.
    pub fn serialize(&self) -> [u8; PUBLIC_KEY_BYTES_LEN] {
        self.point.serialize()
    }

    /// Deserialize `self` from compressed bytes.
    pub fn deserialize(bytes: &[u8]) -> Result<Self, Error> {
        if bytes == &INFINITY_PUBLIC_KEY[..] {
            Err(Error::InvalidInfinityPublicKey)
        } else {
            Ok(Self {
                point: Pub::deserialize(bytes)?,
            })
        }
    }
}

impl GenericPublicKey<PublicKey> {
    /// Generates a new `GenericPublicKey` using cryptographic randomness.
    pub fn random() -> Self {
        let secret_key = BlsSecretKey::random();
        secret_key.public_key()
    }
}

impl<Pub: TPublicKey> Eq for GenericPublicKey<Pub> {}

impl<Pub: TPublicKey> std::hash::Hash for GenericPublicKey<Pub> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.serialize().hash(state);
    }
}

impl From<Vec<u8>> for GenericPublicKey<PublicKey> {
    fn from(bytes: Vec<u8>) -> Self {
        let public_key = PublicKey::from_bytes(&bytes).expect("Failed to parse public key");
        Self { point: public_key }
    }
}

impl From<reth::rpc::types::beacon::BlsPublicKey> for BlsPublicKey {
    fn from(fixed_bytes: reth::rpc::types::beacon::BlsPublicKey) -> Self {
        let bytes = fixed_bytes.to_vec();
        let public_key =
            PublicKey::from_bytes(&bytes).expect("Failed to parse public key from fixed bytes");

        BlsPublicKey { point: public_key }
    }
}

impl<Pub: TPublicKey> PartialEq for GenericPublicKey<Pub> {
    fn eq(&self, other: &Self) -> bool {
        self.serialize()[..] == other.serialize()[..]
    }
}

impl<Pub: TPublicKey> Encode for GenericPublicKey<Pub> {
    impl_ssz_encode!(PUBLIC_KEY_BYTES_LEN);
}

impl<Pub: TPublicKey> Decode for GenericPublicKey<Pub> {
    impl_ssz_decode!(PUBLIC_KEY_BYTES_LEN);
}

impl<Pub: TPublicKey> TreeHash for GenericPublicKey<Pub> {
    impl_tree_hash!(PUBLIC_KEY_BYTES_LEN);
}

impl<Pub: TPublicKey> fmt::Display for GenericPublicKey<Pub> {
    impl_display!();
}

impl<Pub: TPublicKey> std::str::FromStr for GenericPublicKey<Pub> {
    impl_from_str!();
}

impl<Pub: TPublicKey> Serialize for GenericPublicKey<Pub> {
    impl_serde_serialize!();
}

impl<'de, Pub: TPublicKey> Deserialize<'de> for GenericPublicKey<Pub> {
    impl_serde_deserialize!();
}

impl<Pub: TPublicKey> fmt::Debug for GenericPublicKey<Pub> {
    impl_debug!();
}
/// A BLS public key that uses the `blst` library.
pub type BlsPublicKey = GenericPublicKey<PublicKey>;

impl Default for BlsPublicKey {
    fn default() -> Self {
        Self {
            point: PublicKey::default(),
        }
    }
}
