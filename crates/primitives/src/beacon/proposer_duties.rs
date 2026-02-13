use reth::rpc::types::beacon::BlsPublicKey;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;

/// Beacon node response from /eth/v1/validator/duties/proposer/{epoch}
#[derive(Debug, Serialize, Deserialize)]
pub struct ProposerDutiesResponse {
    /// List of proposer duties data.
    pub data: Vec<ProposerDutiesData>,
}

impl ProposerDutiesResponse {
    /// Merges the current proposer duties with another instance's duties,
    /// returning a new `ProposerDutiesResponse` containing the combined duties data.
    pub fn merge_with(self, other: ProposerDutiesResponse) -> Vec<ProposerDutiesData> {
        self.data.into_iter().chain(other.data).collect()
    }
}

/// Proposer duties data in the response from /eth/v1/validator/duties/proposer/{epoch}
#[serde_as]
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct ProposerDutiesData {
    /// The slot number for the proposer duties.
    #[serde_as(as = "DisplayFromStr")]
    pub slot: u64,
    /// The public key of the proposer.
    pub pubkey: BlsPublicKey,
    /// The index of the validator.
    #[serde_as(as = "DisplayFromStr")]
    pub validator_index: u64,
}
