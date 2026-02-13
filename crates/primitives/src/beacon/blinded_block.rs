use crate::blst::SignedRoot;
use crate::{
    blst::signature::BlsSignature,
    types::{ExecutionPayloadHeader, B256},
};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use ssz_types::VariableList;
use tree_hash_derive::TreeHash;
use typenum::{U128, U16, U2, U4096};

use super::beacon_block::{
    Attestation, AttesterSlashing, Deposit, Eth1Data, ProposerSlashing, SignedBlsToExecutionChange,
    SignedVoluntaryExit, SyncAggregate,
};
use super::kzg_commitments::KzgCommitment;

/// Request object of POST `/eth/v1/builder/blinded_blocks`
///
/// See also <https://ethereum.github.io/builder-specs/#/Builder/submitBlindedBlock>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedBlindedBlock {
    /// The bid message.
    pub message: Arc<BlindedBlock>,
    /// The signature for the bid.
    pub signature: BlsSignature,
}

impl SignedBlindedBlock {
    /// Returns the message of the blinded block.
    pub fn message(&self) -> Arc<BlindedBlock> {
        self.message.clone()
    }
    /// Returns the slot of the blinded block.
    pub fn slot(&self) -> u64 {
        self.message.slot
    }
    /// Returns the proposer index of the blinded block.
    pub fn proposer_index(&self) -> u64 {
        self.message.proposer_index
    }
    /// Returns the signature of the blinded block.
    pub fn signature(&self) -> &BlsSignature {
        &self.signature
    }
    /// Returns the block hash of the blinded block.
    pub fn block_hash(&self) -> B256 {
        self.message.body.execution_payload_header.block_hash
    }
}

/// A blinded block is a block without its transactions but with the root of the transactions.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
#[allow(missing_docs)]
pub struct BlindedBlock {
    #[serde_as(as = "DisplayFromStr")]
    pub slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub proposer_index: u64,
    pub parent_root: B256,
    pub state_root: B256,
    pub body: Arc<BlindedBlockBody>,
}

impl SignedRoot for BlindedBlock {}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
#[allow(missing_docs)]
pub struct BlindedBlockBody {
    pub randao_reveal: BlsSignature,
    pub eth1_data: Eth1Data,
    pub graffiti: B256,
    pub proposer_slashings: VariableList<ProposerSlashing, U16>,
    pub attester_slashings: VariableList<AttesterSlashing, U2>,
    pub attestations: VariableList<Attestation, U128>,
    pub deposits: VariableList<Deposit, U16>,
    pub voluntary_exits: VariableList<SignedVoluntaryExit, U16>,
    pub sync_aggregate: SyncAggregate,
    pub execution_payload_header: ExecutionPayloadHeader,
    pub bls_to_execution_changes: VariableList<SignedBlsToExecutionChange, U16>,
    pub blob_kzg_commitments: VariableList<KzgCommitment, U4096>,
}
use tree_hash::TreeHash;
macro_rules! impl_print_tree_hash {
    ($struct_name:ident { $($field_name:ident: $field_type:ty),* $(,)? }) => {
        impl $struct_name {
            /// Print the tree hash of the struct.
            pub fn print_tree_hash(&self) {
                $( println!("{}: {:?}", stringify!($field_name), self.$field_name.tree_hash_root()); )*
            }
        }
    };
  }

impl_print_tree_hash!(BlindedBlock {
    slot: u64,
    proposer_index: u64,
    parent_root: B256,
    state_root: B256,
    body: Arc<BlindedBlockBody>,
});

impl_print_tree_hash!(BlindedBlockBody {
    randao_reveal: BlsSignature,
    eth1_data: Eth1Data,
    graffiti: B256,
    proposer_slashings: List<ProposerSlashing, U16>,
    attester_slashings: List<AttesterSlashing, U2>,
    attestations: List<Attestation, U128>,
    deposits: List<Deposit, U16>,
    voluntary_exits: List<SignedVoluntaryExit, U16>,
    sync_aggregate: SyncAggregate,
    execution_payload_header: ExecutionPayloadHeader,
    bls_to_execution_changes: List<SignedBlsToExecutionChange, U16>,
    blob_kzg_commitments: List<KzgCommitment, U4096>,
});
