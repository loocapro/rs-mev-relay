use c_kzg::BYTES_PER_COMMITMENT;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_utils::hex;
use ssz_derive::{Decode, Encode};
use ssz_types::{FixedVector, VariableList};
use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;
use tree_hash::{PackedEncoding, TreeHash};

use crate::kzg::BYTES_PER_PROOF;
/// A KZG commitment as per EIP-4844.
#[derive(Clone, Copy, Encode, Decode)]
#[ssz(struct_behaviour = "transparent")]
pub struct KzgCommitment(pub [u8; c_kzg::BYTES_PER_COMMITMENT]);

impl From<[u8; BYTES_PER_COMMITMENT]> for KzgCommitment {
    fn from(value: [u8; BYTES_PER_COMMITMENT]) -> Self {
        Self(value)
    }
}

impl From<KzgCommitment> for c_kzg::Bytes48 {
    fn from(value: KzgCommitment) -> Self {
        value.0.into()
    }
}

impl Display for KzgCommitment {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "0x")?;
        for i in &self.0[0..2] {
            write!(f, "{:02x}", i)?;
        }
        write!(f, "…")?;
        for i in &self.0[BYTES_PER_COMMITMENT - 2..BYTES_PER_COMMITMENT] {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

impl TreeHash for KzgCommitment {
    fn tree_hash_type() -> tree_hash::TreeHashType {
        <[u8; BYTES_PER_COMMITMENT] as TreeHash>::tree_hash_type()
    }

    fn tree_hash_packed_encoding(&self) -> PackedEncoding {
        self.0.tree_hash_packed_encoding()
    }

    fn tree_hash_packing_factor() -> usize {
        <[u8; BYTES_PER_COMMITMENT] as TreeHash>::tree_hash_packing_factor()
    }

    fn tree_hash_root(&self) -> tree_hash::Hash256 {
        self.0.tree_hash_root()
    }
}

impl Serialize for KzgCommitment {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:?}", self))
    }
}

impl<'de> Deserialize<'de> for KzgCommitment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;
        Self::from_str(&string).map_err(serde::de::Error::custom)
    }
}

impl Debug for KzgCommitment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", serde_utils::hex::encode(self.0))
    }
}

impl FromStr for KzgCommitment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s).map_err(|e| e.to_string())?;
        if bytes.len() == BYTES_PER_COMMITMENT {
            let mut kzg_commitment_bytes = [0; BYTES_PER_COMMITMENT];
            kzg_commitment_bytes[..].copy_from_slice(&bytes);
            Ok(Self(kzg_commitment_bytes))
        } else {
            Err(format!(
                "InvalidByteLength: got {}, expected {}",
                bytes.len(),
                BYTES_PER_COMMITMENT
            ))
        }
    }
}

#[derive(PartialEq, Hash, Clone, Copy, Encode)]
#[ssz(struct_behaviour = "transparent")]
/// A KZG proof as per EIP-4844.
pub struct KzgProof(pub [u8; BYTES_PER_PROOF]);

/// A blob of data.
pub type Blob = FixedVector<u8, typenum::U131072>;

/// A list of KZG commitments.
pub type KzgProofs = VariableList<KzgProof, typenum::U4096>;
