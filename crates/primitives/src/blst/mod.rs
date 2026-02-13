/// https://github.com/sigp/lighthouse/blob/dcd69dfc628cad5998225d5100b222458f3f0ecb/crypto/bls
/// We can try to remove these files once we are on latest reth and see if lighthouse is compatible
use blst::BLST_ERROR as BlstError;
use reth::primitives::keccak256;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use tree_hash::{Hash256, MerkleHasher, TreeHash, HASHSIZE};
use tree_hash::{PackedEncoding, TreeHashType};
use typenum::Unsigned;

use crate::types::{Address, Bloom, Bytes, B256, U256};

#[macro_use]
/// Macros utilities used to implement some traits.
pub mod macros;
/// BLST public key
pub mod public_key;

/// BLST secret key
pub mod secret_key;
/// BLST signature
pub mod signature;
/// BLST signer
pub mod signer;

#[derive(Clone, Debug, PartialEq)]
/// BLST errors
pub enum Error {
    /// BLST library errors
    Blst(BlstError),
    /// The provided bytes were an incorrect length.
    InvalidByteLength {
        /// The actual length.
        got: usize,
        /// The expected length.
        expected: usize,
    },
    /// The provided secret key bytes were an incorrect length.
    InvalidSecretKeyLength {
        /// The actual length.
        got: usize,
        /// The expected length.
        expected: usize,
    },
    /// The public key represents the point at infinity, which is invalid.
    InvalidInfinityPublicKey,
    /// The secret key is all zero bytes, which is invalid.
    InvalidZeroSecretKey,
}

impl From<BlstError> for Error {
    fn from(e: BlstError) -> Error {
        Error::Blst(e)
    }
}

#[derive(Default, Debug, Serialize, Deserialize, Encode, Decode, tree_hash_derive::TreeHash)]
/// Helper struct for signing data with blst algo.
pub struct SigningData {
    /// The root of the object being signed.
    pub object_root: Hash256,
    /// The domain to sign with.
    pub domain: Hash256,
}

/// Trait for objects that can be signed.
pub trait SignedRoot: TreeHash {
    /// Returns the signing root of the object.
    fn signing_root(&self, domain: Hash256) -> Hash256 {
        SigningData {
            object_root: self.tree_hash_root(),
            domain,
        }
        .tree_hash_root()
    }
}

impl TreeHash for B256 {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Vector
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        PackedEncoding::from_slice(self.as_ref().as_slice())
    }

    fn tree_hash_packing_factor() -> usize {
        1
    }

    fn tree_hash_root(&self) -> Hash256 {
        let me = *self;
        me.into()
    }
}

impl TreeHash for Bloom {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Vector
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        PackedEncoding::from_slice(self.as_ref().as_slice())
    }

    fn tree_hash_packing_factor() -> usize {
        1
    }

    fn tree_hash_root(&self) -> Hash256 {
        let hash = keccak256(self.as_ref());
        Hash256::from_slice(hash.as_slice())
    }
}

impl TreeHash for Bytes {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Vector
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        PackedEncoding::from_slice(self.as_ref())
    }

    fn tree_hash_packing_factor() -> usize {
        1
    }

    fn tree_hash_root(&self) -> Hash256 {
        let hash = keccak256(self.as_ref());
        Hash256::from_slice(hash.as_slice())
    }
}

impl TreeHash for U256 {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Basic
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        let result = self.as_ref().as_le_slice();
        PackedEncoding::from_slice(result)
    }

    fn tree_hash_packing_factor() -> usize {
        1
    }

    fn tree_hash_root(&self) -> Hash256 {
        let result = self.as_ref().as_le_slice();
        Hash256::from_slice(result)
    }
}

impl TreeHash for Address {
    fn tree_hash_type() -> TreeHashType {
        TreeHashType::Vector
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        let mut result = [0; HASHSIZE];
        result[0..20].copy_from_slice(self.as_ref().as_slice());
        PackedEncoding::from_slice(&result)
    }

    fn tree_hash_packing_factor() -> usize {
        1
    }

    fn tree_hash_root(&self) -> Hash256 {
        let mut result = [0; HASHSIZE];
        result[0..20].copy_from_slice(self.as_ref().as_slice());
        Hash256::from_slice(&result)
    }
}

/// A helper function providing common functionality between the `TreeHash` implementations for
/// `FixedVector` and `VariableList`.
pub fn vec_tree_hash_root<T, N>(vec: &[T]) -> Hash256
where
    T: TreeHash,
    N: Unsigned,
{
    match T::tree_hash_type() {
        TreeHashType::Basic => {
            let mut hasher = MerkleHasher::with_leaves(
                N::to_usize().div_ceil(T::tree_hash_packing_factor()),
            );

            for item in vec {
                hasher
                    .write(&item.tree_hash_packed_encoding())
                    .expect("ssz_types variable vec should not contain more elements than max");
            }

            hasher
                .finish()
                .expect("ssz_types variable vec should not have a remaining buffer")
        }
        TreeHashType::Container | TreeHashType::List | TreeHashType::Vector => {
            let mut hasher = MerkleHasher::with_leaves(N::to_usize());

            for item in vec {
                hasher
                    .write(item.tree_hash_root().as_bytes())
                    .expect("ssz_types vec should not contain more elements than max");
            }

            hasher
                .finish()
                .expect("ssz_types vec should not have a remaining buffer")
        }
    }
}

/// The size of a chunk in bytes used within the Merkle tree hashing process.
///
/// This constant defines the granularity at which data is split into chunks before being hashed in the Merkle tree.
/// A chunk is the basic unit of data for which a single hash value is calculated. This size impacts the structure
/// and depth of the Merkle tree, with smaller sizes leading to deeper trees.
pub const BYTES_PER_CHUNK: usize = 32;

/// A helper function providing common functionality for finding the Merkle root of some bytes that
/// represent a bitfield.
pub fn bitfield_bytes_tree_hash_root<N: Unsigned>(bytes: &[u8]) -> Hash256 {
    let byte_size = N::to_usize().div_ceil(8);
    let leaf_count = byte_size.div_ceil(BYTES_PER_CHUNK);

    let mut hasher = MerkleHasher::with_leaves(leaf_count);

    hasher
        .write(bytes)
        .expect("bitfield should not exceed tree hash leaf limit");

    hasher
        .finish()
        .expect("bitfield tree hash buffer should not exceed leaf limit")
}
