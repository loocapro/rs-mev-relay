use super::blinded_block::BlindedBlock;
use crate::beacon::blinded_block::BlindedBlockBody;
use crate::beacon::kzg_commitments::KzgCommitment;
use crate::blst::public_key::BlsPublicKey;
use crate::types::quoted_variable_list_u64;
use crate::types::BlobsBundle;
use crate::types::{Address, ExecutionPayload};
use crate::{blst::signature::BlsSignature, types::B256};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use ssz_derive::Encode;
use ssz_types::{BitList, VariableList};
use ssz_types::{BitVector, FixedVector};
use std::sync::Arc;
use tree_hash_derive::TreeHash;
use typenum::U4096;
use typenum::U512;
use typenum::{U128, U16, U2, U2048, U33};

/// Request object of POST `/eth/v1/builder/blinded_blocks`
///
/// See also <https://ethereum.github.io/builder-specs/#/Builder/submitBlindedBlock>
#[derive(Debug, Clone, Serialize, Deserialize, Encode)]
pub struct SignedBeaconBlock {
    /// The bid message.
    message: Arc<BeaconBlock>,
    /// The signature for the bid.
    signature: BlsSignature,
}

/// Request body for POST `/eth/v1/builder/blocks`
#[derive(Debug, Clone, Serialize, Deserialize, Encode)]
pub struct SignedBeaconBlockContent {
    /// The signed beacon block.
    signed_block: SignedBeaconBlock,
    /// The KZG proofs.
    kzg_proofs: VariableList<KzgCommitment, typenum::U4096>,
    /// The blobs.
    blobs: VariableList<crate::beacon::kzg_commitments::Blob, typenum::U4096>,
}

impl SignedBeaconBlockContent {
    /// Creates a new instance of the signed beacon block content.
    pub fn new(
        execution_payload: Arc<ExecutionPayload>,
        blinded_block: Arc<BlindedBlock>,
        blinded_block_sig: BlsSignature,
        blobs_data: BlobsBundle,
    ) -> Self {
        Self {
            signed_block: SignedBeaconBlock::new(
                execution_payload,
                blinded_block,
                blinded_block_sig,
            ),
            kzg_proofs: blobs_data.proofs,
            blobs: blobs_data.blobs,
        }
    }

    /// Returns the block hash
    pub fn block_hash(&self) -> B256 {
        self.signed_block.message.body.execution_payload.block_hash
    }
    /// Extra data
    pub fn extra_data(&self) -> VariableList<u8, typenum::U32> {
        self.signed_block
            .message
            .body
            .execution_payload
            .extra_data
            .clone()
    }
    /// Returns the slot of the blinded message.
    pub fn slot(&self) -> u64 {
        self.signed_block.message.slot
    }
    /// Returns the proposer index of the blinded message.
    pub fn block_number(&self) -> u64 {
        self.signed_block
            .message
            .body
            .execution_payload
            .block_number
    }
    /// Returns the number of transactions in the message.
    pub fn num_txs(&self) -> usize {
        self.signed_block
            .message
            .body
            .execution_payload
            .transactions
            .len()
    }

    /// Returns the execution header of the beacon message.
    pub fn execution_payload(&self) -> Arc<ExecutionPayload> {
        self.signed_block.message.body.execution_payload.clone()
    }
}

impl SignedBeaconBlock {
    /// Creates a new instance of the signed beacon block from the blinded block.
    pub fn new(
        execution_payload: Arc<ExecutionPayload>,
        blinded_block: Arc<BlindedBlock>,
        blinded_block_sig: BlsSignature,
    ) -> Self {
        let message = Arc::new(BeaconBlock::new(execution_payload, blinded_block));
        let signature = blinded_block_sig;
        Self { message, signature }
    }
}

/// A blinded block is a block without its transactions but with the root of the transactions.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, Encode)]
pub struct BeaconBlock {
    #[serde_as(as = "DisplayFromStr")]
    slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    proposer_index: u64,
    parent_root: B256,
    state_root: B256,
    body: BeaconBlockBody,
}

impl BeaconBlock {
    /// Creates a new BeaconBlock from the given ExecutionPayload and BlindedBlock.
    pub fn new(execution_payload: Arc<ExecutionPayload>, blinded_block: Arc<BlindedBlock>) -> Self {
        let blinded_block = blinded_block.as_ref().clone();
        BeaconBlock {
            slot: blinded_block.slot,
            proposer_index: blinded_block.proposer_index,
            parent_root: blinded_block.parent_root,
            state_root: blinded_block.state_root,
            body: BeaconBlockBody::new(execution_payload, blinded_block.body),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Encode)]
#[allow(missing_docs)]
pub struct BeaconBlockBody {
    randao_reveal: BlsSignature,
    eth1_data: Eth1Data,
    graffiti: B256,
    proposer_slashings: VariableList<ProposerSlashing, U16>,
    attester_slashings: VariableList<AttesterSlashing, U2>,
    attestations: VariableList<Attestation, U128>,
    deposits: VariableList<Deposit, U16>,
    voluntary_exits: VariableList<SignedVoluntaryExit, U16>,
    sync_aggregate: SyncAggregate,
    execution_payload: Arc<ExecutionPayload>,
    bls_to_execution_changes: VariableList<SignedBlsToExecutionChange, U16>,
    blob_kzg_commitments: VariableList<KzgCommitment, U4096>,
}

impl BeaconBlockBody {
    /// Creates a new BeaconBlockBody from the given ExecutionPayload and BlindedBlockBody.
    pub fn new(
        execution_payload: Arc<ExecutionPayload>,
        blinded_block_body: Arc<BlindedBlockBody>,
    ) -> Self {
        let blinded_block_body = blinded_block_body.as_ref().clone();
        BeaconBlockBody {
            randao_reveal: blinded_block_body.randao_reveal,
            eth1_data: blinded_block_body.eth1_data,
            graffiti: blinded_block_body.graffiti,
            proposer_slashings: blinded_block_body.proposer_slashings,
            attester_slashings: blinded_block_body.attester_slashings,
            attestations: blinded_block_body.attestations,
            deposits: blinded_block_body.deposits,
            voluntary_exits: blinded_block_body.voluntary_exits,
            sync_aggregate: blinded_block_body.sync_aggregate,
            execution_payload,
            bls_to_execution_changes: blinded_block_body.bls_to_execution_changes,
            blob_kzg_commitments: blinded_block_body.blob_kzg_commitments,
        }
    }
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct Eth1Data {
    pub deposit_root: B256,
    #[serde_as(as = "DisplayFromStr")]
    pub deposit_count: u64,
    pub block_hash: B256,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct ProposerSlashing {
    signed_header_1: SignedBeaconBlockHeader,
    signed_header_2: SignedBeaconBlockHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct SignedBeaconBlockHeader {
    message: BeaconBlockHeader,
    signature: BlsSignature,
}
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct BeaconBlockHeader {
    #[serde_as(as = "DisplayFromStr")]
    slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    proposer_index: u64,
    #[serde_as(as = "DisplayFromStr")]
    parent_root: B256,
    #[serde_as(as = "DisplayFromStr")]
    state_root: B256,
    #[serde_as(as = "DisplayFromStr")]
    body_root: B256,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct AttesterSlashing {
    attestation_1: IndexedAttestation,
    attestation_2: IndexedAttestation,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct IndexedAttestation {
    #[serde(with = "quoted_variable_list_u64")]
    attesting_indices: VariableList<u64, U2048>,
    data: AttestationData,
    signature: BlsSignature,
}
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct AttestationData {
    #[serde_as(as = "DisplayFromStr")]
    slot: u64,
    #[serde_as(as = "DisplayFromStr")]
    index: u64,
    beacon_block_root: B256,
    source: Checkpoint,
    target: Checkpoint,
}
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct Checkpoint {
    #[serde_as(as = "DisplayFromStr")]
    epoch: u64,
    root: B256,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct Attestation {
    aggregation_bits: BitList<U2048>,
    data: AttestationData,
    signature: BlsSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct Deposit {
    proof: FixedVector<B256, U33>,
    data: DepositData,
}
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct DepositData {
    pubkey: BlsPublicKey,
    withdrawal_credentials: B256,
    #[serde_as(as = "DisplayFromStr")]
    amount: u64,
    signature: BlsSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct SignedVoluntaryExit {
    message: VoluntaryExit,
    signature: BlsSignature,
}
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct VoluntaryExit {
    #[serde_as(as = "DisplayFromStr")]
    epoch: u64,
    #[serde_as(as = "DisplayFromStr")]
    validator_index: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct SyncAggregate {
    sync_committee_bits: BitVector<U512>,
    sync_committee_signature: BlsSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct SignedBlsToExecutionChange {
    message: BlsToExecutionChange,
    signature: BlsSignature,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash, Encode)]
#[allow(missing_docs)]
pub struct BlsToExecutionChange {
    #[serde_as(as = "DisplayFromStr")]
    validator_index: u64,
    from_bls_pubkey: BlsPublicKey,
    to_execution_address: Address,
}
