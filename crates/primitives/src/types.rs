use alloy_primitives::hex::hex;
use alloy_primitives::hex::FromHexError;
use alloy_rlp::RlpDecodable;
use alloy_rlp::RlpDecodableWrapper;
use alloy_rlp::RlpEncodable;
use alloy_rlp::RlpEncodableWrapper;
use core::fmt::Formatter;
use reth::primitives::alloy_primitives::private::alloy_rlp::Decodable;
use reth::primitives::{
    alloy_primitives::BLOOM_SIZE_BYTES, kzg::BYTES_PER_BLOB, TransactionSigned,
};
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use ssz::Encode;
use ssz_derive::Encode;
use ssz_types::FixedVector;
use ssz_types::VariableList;
use std::fmt;
use std::fmt::Display;
use std::str::FromStr;
use tree_hash::{Hash256, HASHSIZE};
use tree_hash_derive::TreeHash;
use typenum::Unsigned;
use typenum::U32;
use warp::reject::Reject;

use crate::beacon::kzg_commitments::KzgCommitment;
#[derive(Debug, Clone, Copy, Eq, Serialize, Deserialize, Hash, PartialEq)]
/// B256 implementation type
pub struct B256(reth::primitives::B256);

impl Encode for B256 {
    fn is_ssz_fixed_len() -> bool {
        <Hash256 as Encode>::is_ssz_fixed_len()
    }
    fn ssz_fixed_len() -> usize {
        <Hash256 as Encode>::ssz_fixed_len()
    }
    fn ssz_bytes_len(&self) -> usize {
        self.0.ssz_bytes_len()
    }
    fn ssz_append(&self, buf: &mut Vec<u8>) {
        self.0.ssz_append(buf)
    }
}

impl From<Hash256> for B256 {
    fn from(hash: Hash256) -> Self {
        let bytes: [u8; 32] = hash.into();
        Self(reth::primitives::B256::from_slice(&bytes))
    }
}

impl Display for B256 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl B256 {
    /// Returns the zero value.
    pub fn zero() -> Self {
        Self(reth::primitives::B256::ZERO)
    }
    /// Returns a random value.
    pub fn random() -> Self {
        Self(reth::primitives::B256::random())
    }
    /// Returns the value as a byte array.
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl FromStr for B256 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = reth::primitives::B256::from_str(s)
            .map_err(|e| format!("Failed to parse B256: {:?}", e))?;
        Ok(Self(value))
    }
}

impl From<reth::primitives::B256> for B256 {
    fn from(alloy: reth::primitives::B256) -> Self {
        Self(alloy)
    }
}

impl AsRef<reth::primitives::B256> for B256 {
    fn as_ref(&self) -> &reth::primitives::B256 {
        &self.0
    }
}

/// Errors that can occur while converting grpc types to relay types.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConversionError {
    /// Error indicating an incorrect length for a specific data type.
    #[error("{data_type} length mismatch: expected {expected}, got {actual}")]
    IncorrectLength {
        /// The type of data for which the length is incorrect.
        data_type: String,
        /// The expected length.
        expected: usize,
        /// The actual length encountered.
        actual: usize,
    },
    /// Error indicating failure to parse a specific value, providing the offending input.
    #[error("Failed to parse {data_type} from provided value: {value}")]
    ParseError {
        /// The type of data attempted to be parsed.
        data_type: String,
        /// The value that failed to parse.
        value: String,
    },

    /// Error indicating failure to parse bytes to tx.
    #[error("Failed to parse bytes to tx")]
    AlloyRlp(#[from] alloy_rlp::Error),
    /// Error indicating failure to parse ssz types.
    #[error("Failed to parse ssz types")]
    SszTypes,
}

impl Reject for ConversionError {}

impl TryFrom<Vec<u8>> for B256 {
    type Error = ConversionError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != HASHSIZE {
            return Err(ConversionError::IncorrectLength {
                expected: HASHSIZE,
                actual: bytes.len(),
                data_type: "HASH".to_string(),
            });
        }
        Ok(Self(reth::primitives::B256::from_slice(&bytes)))
    }
}

impl From<B256> for Hash256 {
    fn from(alloy: B256) -> Self {
        let bytes: [u8; 32] = alloy.0.into();
        Self::from(bytes)
    }
}

/// U256 implementation type
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct U256(reth::primitives::U256);

impl Encode for U256 {
    fn is_ssz_fixed_len() -> bool {
        true
    }
    fn ssz_fixed_len() -> usize {
        32
    }
    fn ssz_bytes_len(&self) -> usize {
        32
    }
    fn ssz_append(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.0.as_le_slice());
    }
}

impl Display for U256 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<reth::primitives::U256> for U256 {
    fn as_ref(&self) -> &reth::primitives::U256 {
        &self.0
    }
}

impl From<u64> for U256 {
    fn from(value: u64) -> Self {
        Self(reth::primitives::U256::from(value))
    }
}

impl TryFrom<Vec<u8>> for U256 {
    type Error = ConversionError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != HASHSIZE {
            return Err(ConversionError::IncorrectLength {
                expected: HASHSIZE,
                actual: bytes.len(),
                data_type: "U256".to_string(),
            });
        }
        Ok(Self(reth::primitives::U256::from_be_slice(&bytes)))
    }
}

impl FromStr for U256 {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = reth::primitives::U256::from_str(s)
            .map_err(|e| format!("Failed to parse U256: {:?}", e))?;
        Ok(Self(value))
    }
}
/// Address implementation type
#[derive(
    Debug,
    Clone,
    Eq,
    Copy,
    PartialEq,
    Serialize,
    RlpDecodableWrapper,
    RlpEncodableWrapper,
    Deserialize,
)]
pub struct Address(reth::primitives::Address);

impl FromStr for Address {
    type Err = FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(reth::primitives::Address::from_str(s)?))
    }
}

impl Encode for Address {
    fn is_ssz_fixed_len() -> bool {
        true
    }

    fn ssz_fixed_len() -> usize {
        20
    }

    fn ssz_bytes_len(&self) -> usize {
        20
    }

    fn ssz_append(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.0.as_slice());
    }
}

impl Display for Address {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl Address {
    /// Returns the zero address.
    pub fn zero() -> Self {
        Self(reth::primitives::Address::ZERO)
    }
    /// Returns a random address.
    pub fn random() -> Self {
        Self(reth::primitives::Address::random())
    }
    /// Returns the address as a byte array.
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}
impl From<reth::primitives::Address> for Address {
    fn from(address: reth::primitives::Address) -> Self {
        Self(address)
    }
}

impl AsRef<reth::primitives::Address> for Address {
    fn as_ref(&self) -> &reth::primitives::Address {
        &self.0
    }
}

impl TryFrom<Vec<u8>> for Address {
    type Error = ConversionError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != 20 {
            return Err(ConversionError::IncorrectLength {
                expected: 20,
                actual: bytes.len(),
                data_type: "Address".to_string(),
            });
        }
        Ok(Self(reth::primitives::Address::from_slice(&bytes)))
    }
}

/// Ethereum 256 byte bloom filter.
#[derive(Debug, Clone, Serialize, Deserialize, Hash)]
pub struct Bloom(reth::primitives::Bloom);

impl Bloom {
    /// Initializes a new bloom filter.
    pub fn new(bytes: [u8; 256]) -> Self {
        Self(reth::primitives::Bloom::new(bytes))
    }
}

impl AsRef<reth::primitives::Bloom> for Bloom {
    fn as_ref(&self) -> &reth::primitives::Bloom {
        &self.0
    }
}

impl TryFrom<Vec<u8>> for Bloom {
    type Error = ConversionError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != BLOOM_SIZE_BYTES {
            return Err(ConversionError::IncorrectLength {
                expected: BLOOM_SIZE_BYTES,
                actual: bytes.len(),
                data_type: "Bloom".to_string(),
            });
        }
        Ok(Self(reth::primitives::Bloom::from_slice(&bytes)))
    }
}

/// Wrapper type around [`bytes::Bytes`] to support "0x" prefixed hex strings.
#[derive(Debug, Clone, Serialize, Default, Deserialize)]
pub struct Bytes(reth::primitives::Bytes);

impl FromStr for Bytes {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes = hex::decode(s.trim_start_matches("0x"))
            .map_err(|e| format!("Failed to parse Bytes: {:?}", e))?;
        Ok(Self(reth::primitives::Bytes::copy_from_slice(&bytes)))
    }
}

impl AsMut<reth::primitives::Bytes> for Bytes {
    fn as_mut(&mut self) -> &mut reth::primitives::Bytes {
        &mut self.0
    }
}

impl Bytes {
    /// Try converting the bytes to a signed transaction.
    pub fn try_into_signed_tx(&self) -> Result<TransactionSigned, alloy_rlp::Error> {
        let bytes = self.0.as_ref();
        TransactionSigned::decode(&mut &bytes[..])
    }
}

impl AsRef<reth::primitives::Bytes> for Bytes {
    fn as_ref(&self) -> &reth::primitives::Bytes {
        &self.0
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self(reth::primitives::Bytes::copy_from_slice(&bytes))
    }
}

/// A commitment/proof serialized as 0x-prefixed hex string
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bytes48(reth::rpc::types::kzg::Bytes48);

impl AsRef<reth::rpc::types::kzg::Bytes48> for Bytes48 {
    fn as_ref(&self) -> &reth::rpc::types::kzg::Bytes48 {
        &self.0
    }
}

/// Helper trait to convert between `Bytes48` and `KzgCommitment`.
pub trait AsKzgCommitment {
    /// Convert the type to a `KzgCommitment`.
    fn as_kzg_commitment(&self) -> KzgCommitment;
    /// Convert a `KzgCommitment` to the type.
    fn from_kzg_commitment(commitment: &KzgCommitment) -> Self;
}

impl AsKzgCommitment for Bytes48 {
    fn as_kzg_commitment(&self) -> KzgCommitment {
        // Assuming Bytes48 internally holds a reth::rpc::types::kzg::Bytes48
        // which can be converted into a [u8; 48] or similar.
        // This will need to be adjusted based on your actual internal representation.
        let bytes: [u8; 48] = Into::<[u8; 48]>::into(*self.as_ref());
        KzgCommitment(bytes)
    }

    fn from_kzg_commitment(commitment: &KzgCommitment) -> Self {
        // Here we assume `Bytes48` can be constructed directly from a [u8; 48].
        // You'll need to adjust this based on the actual constructors available for Bytes48.
        Self(reth::rpc::types::kzg::Bytes48::from_slice(&commitment.0))
    }
}

impl TryFrom<Vec<u8>> for Bytes48 {
    type Error = ConversionError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != 48 {
            return Err(ConversionError::IncorrectLength {
                expected: 48,
                actual: bytes.len(),
                data_type: "Bytes48".to_string(),
            });
        }
        Ok(Self(reth::rpc::types::kzg::Bytes48::from_slice(&bytes)))
    }
}

/// A Blob serialized as 0x-prefixed hex string
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blob(reth::rpc::types::kzg::Blob);

impl AsRef<reth::rpc::types::kzg::Blob> for Blob {
    fn as_ref(&self) -> &reth::rpc::types::kzg::Blob {
        &self.0
    }
}

impl TryFrom<Vec<u8>> for Blob {
    type Error = ConversionError;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if bytes.len() != BYTES_PER_BLOB {
            return Err(ConversionError::IncorrectLength {
                expected: BYTES_PER_BLOB,
                actual: bytes.len(),
                data_type: "Blob".to_string(),
            });
        }
        Ok(Self(reth::rpc::types::kzg::Blob::from_slice(&bytes)))
    }
}

/// Withdrawal represents a validator withdrawal from the consensus layer.
#[serde_as]
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, RlpEncodable, TreeHash, RlpDecodable, Encode,
)]
pub struct Withdrawal {
    /// Monotonically increasing identifier issued by consensus layer.
    #[serde_as(as = "DisplayFromStr")]
    pub index: u64,
    /// Index of validator associated with withdrawal.
    #[serde_as(as = "DisplayFromStr")]
    pub validator_index: u64,
    /// Target address for withdrawn ether.
    pub address: Address,
    /// Value of the withdrawal in gwei.
    #[serde_as(as = "DisplayFromStr")]
    pub amount: u64,
}

/// Execution payload.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct ExecutionPayload {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    #[serde(with = "ssz_types::serde_utils::hex_fixed_vec")]
    pub logs_bloom: FixedVector<u8, typenum::U256>,
    pub prev_randao: B256,
    #[serde_as(as = "DisplayFromStr")]
    pub block_number: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub gas_limit: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub gas_used: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: u64,
    #[serde(with = "ssz_types::serde_utils::hex_var_list")]
    pub extra_data: VariableList<u8, U32>,
    #[serde_as(as = "DisplayFromStr")]
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    #[serde(with = "ssz_types::serde_utils::list_of_hex_var_list")]
    pub transactions: VariableList<VariableList<u8, typenum::U1073741824>, typenum::U1048576>,
    pub withdrawals: VariableList<Withdrawal, typenum::U16>,
    #[serde_as(as = "DisplayFromStr")]
    pub blob_gas_used: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub excess_blob_gas: u64,
}

/// Serializes the `logs_bloom` field of an `ExecutionPayload`.
pub mod serde_logs_bloom {
    use super::*;

    use serde_utils::hex::PrefixedHexVisitor;

    /// Serialize a logs bloom as a 0x-prefixed hex string.
    pub fn serialize<S, U>(bytes: &FixedVector<u8, U>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        U: Unsigned,
    {
        let mut hex_string: String = "0x".to_string();
        hex_string.push_str(&hex::encode(&bytes[..]));

        serializer.serialize_str(&hex_string)
    }

    /// Deserialize a logs bloom from a 0x-prefixed hex string.
    pub fn deserialize<'de, D, U>(deserializer: D) -> Result<FixedVector<u8, U>, D::Error>
    where
        D: Deserializer<'de>,
        U: Unsigned,
    {
        let vec = deserializer.deserialize_string(PrefixedHexVisitor)?;

        FixedVector::new(vec)
            .map_err(|e| serde::de::Error::custom(format!("invalid logs bloom: {:?}", e)))
    }
}
/// Execution payload.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
#[allow(missing_docs)]
pub struct ExecutionPayloadHeader {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    #[serde(with = "serde_logs_bloom")]
    pub logs_bloom: FixedVector<u8, typenum::U256>,
    pub prev_randao: B256,
    #[serde_as(as = "DisplayFromStr")]
    pub block_number: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub gas_limit: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub gas_used: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub timestamp: u64,
    #[serde(with = "ssz_types::serde_utils::hex_var_list")]
    pub extra_data: VariableList<u8, U32>,
    #[serde_as(as = "DisplayFromStr")]
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions_root: B256,
    pub withdrawals_root: B256,
    #[serde_as(as = "DisplayFromStr")]
    pub blob_gas_used: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub excess_blob_gas: u64,
}

/// Blobs bundle.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlobsBundle {
    /// All commitments in the bundle.
    pub commitments: Vec<Bytes48>,
    /// All proofs in the bundle.
    pub proofs: VariableList<KzgCommitment, typenum::U4096>,
    /// All blobs in the bundle.
    pub blobs: VariableList<crate::beacon::kzg_commitments::Blob, typenum::U4096>,
}

/// Implementing serializing and deserializing for `List` of `u64`.
pub mod quoted_variable_list_u64 {
    use super::*;
    use serde::ser::SerializeSeq;
    use serde_utils::quoted_u64_vec::{QuotedIntVecVisitor, QuotedIntWrapper};
    /// Helper method to serialize a list of u64 as a quoted integer.
    pub fn serialize<S, T>(value: &VariableList<u64, T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Unsigned,
    {
        let mut seq = serializer.serialize_seq(Some(value.len()))?;
        for &int in value.iter() {
            seq.serialize_element(&QuotedIntWrapper { int })?;
        }
        seq.end()
    }
    /// Helper method to deserialize a list of u64 as a quoted integer.
    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<VariableList<u64, T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Unsigned,
    {
        deserializer
            .deserialize_any(QuotedIntVecVisitor)
            .and_then(|vec| {
                VariableList::new(vec)
                    .map_err(|e| serde::de::Error::custom(format!("invalid length: {:?}", e)))
            })
    }
}
