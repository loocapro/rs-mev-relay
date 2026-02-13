#![allow(
    unused_variables,
    dead_code,
    missing_docs,
    clippy::let_unit_value,
    unreachable_pub
)]
mod mevrelay;
pub use mevrelay::*;
use relay_primitives::beacon::kzg_commitments::Blob;
use relay_primitives::beacon::kzg_commitments::KzgCommitment;
use relay_primitives::kzg::BYTES_PER_COMMITMENT;
use relay_primitives::types::{ConversionError, U256};
use ssz_types::{FixedVector, VariableList};
use std::str::FromStr;

impl TryFrom<crate::gen::BidTrace> for relay_primitives::storage::bid_traces::BidTrace {
    type Error = ConversionError;

    fn try_from(bid_trace: crate::gen::BidTrace) -> Result<Self, Self::Error> {
        let value = U256::from_str(&bid_trace.value).map_err(|_| ConversionError::ParseError {
            data_type: "U256".to_string(),
            value: bid_trace.value,
        })?;

        Ok(relay_primitives::storage::bid_traces::BidTrace {
            slot: bid_trace.slot,
            parent_hash: bid_trace.parent_hash.try_into()?,
            block_hash: bid_trace.block_hash.try_into()?,
            builder_pubkey: bid_trace.builder_pubkey.into(),
            proposer_pubkey: bid_trace.proposer_pubkey.into(),
            proposer_fee_recipient: bid_trace.proposer_fee_recipient.try_into()?,
            gas_limit: bid_trace.gas_limit,
            gas_used: bid_trace.gas_used,
            value,
        })
    }
}

impl TryFrom<crate::gen::Withdrawal> for relay_primitives::types::Withdrawal {
    type Error = ConversionError;
    fn try_from(withdrawal: crate::gen::Withdrawal) -> Result<Self, Self::Error> {
        Ok(relay_primitives::types::Withdrawal {
            index: withdrawal.index,
            validator_index: withdrawal.amount,
            address: withdrawal.address.try_into()?,
            amount: withdrawal.amount,
        })
    }
}
type BlobProofs = Result<Vec<KzgCommitment>, ConversionError>;
type Blobs = Result<Vec<Blob>, ConversionError>;

impl TryFrom<crate::gen::BlobsBundle> for relay_primitives::types::BlobsBundle {
    type Error = ConversionError;

    fn try_from(blobs_bundle: crate::gen::BlobsBundle) -> Result<Self, Self::Error> {
        let commitments = blobs_bundle
            .commitments
            .into_iter()
            .map(|c| c.try_into())
            .collect::<Result<Vec<_>, _>>()?;

        let proofs: BlobProofs = blobs_bundle
            .proofs
            .into_iter()
            .map(|p| {
                let bytes: Result<[u8; BYTES_PER_COMMITMENT], _> =
                    p.try_into().map_err(|_| ConversionError::SszTypes);

                bytes.map(KzgCommitment::from)
            })
            .collect();
        let proofs = proofs?;

        let blobs: Blobs = blobs_bundle
            .blobs
            .into_iter()
            .map(|vec_u8| FixedVector::new(vec_u8.clone()).map_err(|_| ConversionError::SszTypes))
            .collect();

        let blobs = blobs?;

        Ok(relay_primitives::types::BlobsBundle {
            commitments,
            proofs: VariableList::new(proofs).map_err(|_| ConversionError::SszTypes)?,
            blobs: VariableList::new(blobs).map_err(|_| ConversionError::SszTypes)?,
        })
    }
}

impl TryFrom<crate::gen::ExecutionPayload> for relay_primitives::types::ExecutionPayload {
    type Error = ConversionError;
    fn try_from(payload: crate::gen::ExecutionPayload) -> Result<Self, Self::Error> {
        let withdrawals: Result<Vec<_>, _> = payload
            .withdrawals
            .into_iter()
            .map(|w| w.try_into())
            .collect();
        let withdrawals = withdrawals?;

        Ok(relay_primitives::types::ExecutionPayload {
            parent_hash: payload.parent_hash.try_into()?,
            fee_recipient: payload.fee_recipient.clone().try_into().map_err(|_| {
                ConversionError::IncorrectLength {
                    expected: 20,
                    actual: payload.fee_recipient.len(),
                    data_type: "Address".to_string(),
                }
            })?,
            state_root: payload.state_root.try_into()?,
            receipts_root: payload.receipts_root.try_into()?,
            logs_bloom: payload.logs_bloom.clone().into(),
            prev_randao: payload.prev_randao.try_into()?,
            block_number: payload.block_number,
            gas_limit: payload.gas_limit,
            gas_used: payload.gas_used,
            timestamp: payload.timestamp,
            extra_data: payload.extra_data.into(),
            base_fee_per_gas: payload.base_fee_per_gas.try_into()?,
            block_hash: payload.block_hash.try_into()?,
            transactions: VariableList::new(
                payload
                    .transactions
                    .into_iter()
                    .map(|t| t.raw_data.into())
                    .collect(),
            )
            .map_err(|_| ConversionError::SszTypes)?,
            withdrawals: VariableList::new(withdrawals).map_err(|_| ConversionError::SszTypes)?,
            blob_gas_used: payload.blob_gas_used,
            excess_blob_gas: payload.excess_blob_gas,
        })
    }
}
