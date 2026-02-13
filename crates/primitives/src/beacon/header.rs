use super::kzg_commitments::KzgCommitment;
use crate::{
    blst::{public_key::BlsPublicKey, signature::BlsSignature, signer::BlsSigner, SignedRoot},
    storage::bid_submission::BidSubmission,
    types::{
        AsKzgCommitment, ConversionError, ExecutionPayload, ExecutionPayloadHeader, B256, U256,
    },
};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use ssz_types::VariableList;
use tree_hash::TreeHash;
use tree_hash_derive::TreeHash;
use typenum::U4096;

/// Response object of GET `/eth/v1/builder/header/{slot}/{parent_hash}/{pubkey}`
///
/// See also <https://ethereum.github.io/builder-specs/#/Builder/getHeader>
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedHeader {
    /// The bid message.
    pub message: Header,
    /// The signature for the bid.
    pub signature: BlsSignature,
}

/// A builder bid.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, TreeHash)]
pub struct Header {
    /// The execution payload.
    pub header: ExecutionPayloadHeader,
    /// The KZG commitments of the blobs.
    pub blob_kzg_commitments: VariableList<KzgCommitment, U4096>,
    /// The value of the bid.
    #[serde_as(as = "DisplayFromStr")]
    pub value: U256,
    /// The public key of the bidder.
    pub pubkey: BlsPublicKey,
}

impl SignedRoot for Header {}

impl From<ExecutionPayload> for ExecutionPayloadHeader {
    fn from(payload: ExecutionPayload) -> Self {
        let tx_root = payload.transactions.tree_hash_root();
        let withdrawls_root = B256::from(payload.withdrawals.tree_hash_root());

        ExecutionPayloadHeader {
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
            withdrawals_root: withdrawls_root,
            blob_gas_used: payload.blob_gas_used,
            excess_blob_gas: payload.excess_blob_gas,
        }
    }
}

impl TryFrom<(BidSubmission, BlsSigner)> for SignedHeader {
    type Error = ConversionError;
    fn try_from(tuple: (BidSubmission, BlsSigner)) -> Result<SignedHeader, Self::Error> {
        let (submission, signer) = tuple;

        let value = submission.bid_trace().value;
        let exec_payload = submission.execution_payload().as_ref().clone();
        let blobs_bundle = submission.blobs_bundle().as_ref().clone();
        let header = exec_payload.into();

        let kzg_commitments: Vec<KzgCommitment> = blobs_bundle
            .commitments
            .iter()
            .map(|bytes48| bytes48.as_kzg_commitment())
            .collect();

        let blob_kzg_commitments =
            VariableList::new(kzg_commitments).map_err(|_| ConversionError::SszTypes)?;

        let pubkey = signer.public_key();

        let mut msg = Header {
            header,
            value,
            pubkey,
            blob_kzg_commitments,
        };
        let signature = signer.sign(&mut msg);

        Ok(Self {
            message: msg,
            signature,
        })
    }
}
