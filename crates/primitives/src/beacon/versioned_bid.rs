use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use crate::{
    blst::{public_key::BlsPublicKey, signature::BlsSignature},
    storage::bid_submission::BidSubmission,
    types::{ConversionError, ExecutionPayload, ExecutionPayloadHeader, U256},
};

use reth::primitives::{
    proofs::{calculate_transaction_root, ordered_trie_root},
    TransactionSigned,
};
use serde::{Deserialize, Serialize};

/// The name of a fork.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub enum ForkName {
    /// The phase0 fork.
    Base,
    /// The altair fork.
    Altair,
    /// The bellatrix fork.
    Merge,
    /// The capella fork.
    Capella,
    /// The deneb fork.
    Deneb,
}

impl FromStr for ForkName {
    type Err = String;

    fn from_str(fork_name: &str) -> Result<Self, String> {
        Ok(match fork_name.to_lowercase().as_ref() {
            "phase0" | "base" => ForkName::Base,
            "altair" => ForkName::Altair,
            "bellatrix" | "merge" => ForkName::Merge,
            "capella" => ForkName::Capella,
            "deneb" => ForkName::Deneb,
            _ => return Err(format!("unknown fork name: {}", fork_name)),
        })
    }
}

impl TryFrom<String> for ForkName {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_str(&s)
    }
}

impl Display for ForkName {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            ForkName::Base => "phase0".fmt(f),
            ForkName::Altair => "altair".fmt(f),
            ForkName::Merge => "bellatrix".fmt(f),
            ForkName::Capella => "capella".fmt(f),
            ForkName::Deneb => "deneb".fmt(f),
        }
    }
}

impl From<ForkName> for String {
    fn from(fork: ForkName) -> String {
        fork.to_string()
    }
}

/// Response object of GET `/eth/v1/builder/header/{slot}/{parent_hash}/{pubkey}`
///
/// See also <https://ethereum.github.io/builder-specs/#/Builder/getHeader>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedSignedBuilderBid {
    /// The version of the bid.
    pub version: ForkName,
    /// The bid for the base fork.
    pub data: SignedBuilderBid,
}

impl VersionedSignedBuilderBid {
    /// Create a new instance of the versioned bid.
    pub fn new(version: ForkName, data: SignedBuilderBid) -> Self {
        Self { version, data }
    }
}

/// A signed builder bid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedBuilderBid {
    /// The bid message.
    pub message: BuilderBid,
    /// The signature for the bid.
    pub signature: BlsSignature,
}

/// A builder bid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderBid {
    /// The execution payload.
    pub header: ExecutionPayloadHeader,
    /// The value of the bid.
    pub value: U256,
    /// The public key of the bidder.
    pub pubkey: BlsPublicKey,
}

impl TryFrom<ExecutionPayload> for ExecutionPayloadHeader {
    type Error = ConversionError;
    fn try_from(payload: ExecutionPayload) -> Result<Self, Self::Error> {
        let txs: Result<Vec<TransactionSigned>, _> = payload
            .transactions
            .iter()
            .map(|t| t.try_into_signed_tx())
            .collect();
        let tx_root = calculate_transaction_root(&txs?);

        let withdrawls_root = ordered_trie_root(&payload.withdrawals);
        Ok(ExecutionPayloadHeader {
            parent_hash: payload.parent_hash,
            fee_recipient: payload.fee_recipient,
            state_root: payload.state_root,
            receipts_root: payload.receipts_root,
            logs_bloom: payload.logs_bloom,
            prev_randao: payload.prev_randao,
            block_number: payload.block_number,
            gas_limit: payload.gas_limit,
            gas_used: payload.gas_used,
            timestamp: payload.timestamp,
            extra_data: payload.extra_data,
            base_fee_per_gas: payload.base_fee_per_gas,
            block_hash: payload.block_hash,
            transactions_root: tx_root.into(),
            withdrawals_root: withdrawls_root.into(),
            blob_gas_used: payload.blob_gas_used,
            excess_blob_gas: payload.excess_blob_gas,
        })
    }
}

impl TryFrom<BidSubmission> for SignedBuilderBid {
    type Error = ConversionError;
    fn try_from(submission: BidSubmission) -> Result<SignedBuilderBid, Self::Error> {
        let signature = submission.signature().clone();
        let value = submission.bid_trace().value;
        let pubkey = submission.bid_trace().builder_pubkey;
        let header = submission.execution_payload().try_into()?;

        Ok(Self {
            message: BuilderBid {
                header,
                value,
                pubkey,
            },
            signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Address, Bloom, Bytes, B256};
    use alloy_primitives::bytes;
    use reth::primitives::hex;
    use std::str::FromStr;

    #[test]
    fn exec_payload_to_exec_header() {
        let exec_payload = execution_payload();
        let parent_hash = exec_payload.parent_hash;
        let fee_recipient = exec_payload.fee_recipient;
        let state_root = exec_payload.state_root;
        let receipts_root = exec_payload.receipts_root;
        let prev_randao = exec_payload.prev_randao;
        let exec_header: ExecutionPayloadHeader = exec_payload.try_into().unwrap();
        assert_eq!(exec_header.parent_hash, parent_hash);
        assert_eq!(exec_header.fee_recipient, fee_recipient);
        assert_eq!(exec_header.state_root, state_root);
        assert_eq!(exec_header.receipts_root, receipts_root);
        assert_eq!(exec_header.prev_randao, prev_randao);
    }

    fn execution_payload() -> ExecutionPayload {
        let logs_bloom: reth::primitives::Bloom = hex!("002400000000004000220000800002000000000000000000000000000000100000000000000000100000000000000021020000000800000006000000002100040000000c0004000000000008000008200000000000000000000000008000000001040000020000020000002000000800000002000020000000022010000000000000010002001000000000020200000000000001000200880000004000000900020000000000020000000040000000000000000000000000000080000000000001000002000000000000012000200020000000000000001000000000000020000010321400000000100000000000000000000000000000400000000000000000").into();
        let tx = Bytes::from_str("f861806482520894c245f4624f46f81024f67e7f10790a01bd7a1b3a648082fd20a0bbe2c68866e424d1c41ff602872cbe18406d1b1c81e8807fd31304b769d4f744a04fd6358a20e05661f9d6f7044aaeefe055eff2be7507f766b2bf0bfe1c3b959c").unwrap();

        ExecutionPayload {
            block_number: 10,
            gas_limit: 1310,
            gas_used: 110,
            timestamp: 1132130,
            extra_data: bytes!("7465737400000000000000000000000000000000000000000000000000000000")
                .to_vec()
                .into(),
            base_fee_per_gas: U256::from(1000),
            block_hash: B256::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            transactions: vec![tx],
            withdrawals: vec![],
            blob_gas_used: 0,
            excess_blob_gas: 0,
            parent_hash: B256::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            prev_randao: B256::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            fee_recipient: Address::random(),
            state_root: B256::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            receipts_root: B256::from_str(
                "0x0000000000000000000000000000000000000000000000000000000000000001",
            )
            .unwrap(),
            logs_bloom: Bloom::new(**logs_bloom),
        }
    }
}
