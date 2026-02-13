use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::bid_traces::BidTrace;
use crate::{
    beacon::BlindedBlockResponse,
    blst::signature::BlsSignature,
    types::{BlobsBundle, ExecutionPayload},
};

/// A block builder bid submission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BidSubmission {
    /// The bid trace.
    message: BidTrace,
    /// The execution payload.
    execution_payload: Arc<ExecutionPayload>,
    /// The blobs bundle.
    blobs_bundle: Arc<BlobsBundle>,
    /// The signature for the bid.
    signature: BlsSignature,
}

impl BidSubmission {
    /// Create a new instance of the bid submission.
    pub fn new(
        bid: BidTrace,
        execution_payload: Arc<ExecutionPayload>,
        blobs_bundle: Arc<BlobsBundle>,
        signature: BlsSignature,
    ) -> Self {
        Self {
            message: bid,
            execution_payload,
            blobs_bundle,
            signature,
        }
    }

    /// Returns the bid trace.
    pub fn bid_trace(&self) -> &BidTrace {
        &self.message
    }
    /// Returns the signature of the submission.
    pub fn signature(&self) -> &BlsSignature {
        &self.signature
    }
    /// Returns the execution payload.
    pub fn execution_payload(&self) -> Arc<ExecutionPayload> {
        self.execution_payload.clone()
    }
    /// Returns the Blinded block response.
    pub fn to_blinded_block_response(&self) -> BlindedBlockResponse {
        BlindedBlockResponse {
            execution_payload: self.execution_payload.clone(),
            blobs_bundle: self.blobs_bundle.clone(),
        }
    }

    /// Returns the blobs bundle.
    pub fn blobs_bundle(&self) -> Arc<BlobsBundle> {
        self.blobs_bundle.clone()
    }
}
