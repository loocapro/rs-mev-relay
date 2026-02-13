use std::cmp::Ordering;

use crate::blst::public_key::BlsPublicKey;
use crate::blst::signature::BlsSignature;
use crate::blst::SignedRoot;
use crate::types::{Address, B256, U256};
use serde::Deserialize;
use serde_derive::Serialize;
use serde_with::{serde_as, DisplayFromStr};
use tree_hash_derive::TreeHash;

/// The Bid trace type.
#[serde_as]
#[derive(Debug, Clone, TreeHash, Serialize, Deserialize, PartialEq, Eq)]
pub struct BidTrace {
    /// The slot associated with the block.
    #[serde_as(as = "DisplayFromStr")]
    pub slot: u64,
    /// The parent hash of the block.
    pub parent_hash: B256,
    /// The hash of the block.
    pub block_hash: B256,
    /// The public key of the builder.
    pub builder_pubkey: BlsPublicKey,
    /// The public key of the proposer.
    pub proposer_pubkey: BlsPublicKey,
    /// The recipient of the proposer's fee.
    pub proposer_fee_recipient: Address,
    /// The gas limit associated with the block.
    #[serde_as(as = "DisplayFromStr")]
    pub gas_limit: u64,
    /// The gas used within the block.
    #[serde_as(as = "DisplayFromStr")]
    pub gas_used: u64,
    /// The value associated with the block.
    pub value: U256,
}

impl BidTrace {
    /// Verify the signature of the bid trace.
    pub fn verify_signature(&self, signature: &BlsSignature, domain: [u8; 32]) -> bool {
        let signing_root = self.signing_root(domain.into());
        signature.verify(&self.builder_pubkey, signing_root)
    }
}

impl PartialOrd for BidTrace {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BidTrace {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl SignedRoot for BidTrace {}
