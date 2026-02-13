use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::types::{BlobsBundle, ExecutionPayload};

use self::fork::ForkName;

/// Beacon node types related to known validator of the consensus.
pub mod known_validator;
/// Beacon node types related to proposer duties.
pub mod proposer_duties;
/// Beacon node types related to proposer beacon sync status.
pub mod sync_status;

/// Beacon node events.
pub mod events;

/// Beacon validator registrations.
pub mod registrations;

/// A versioned bid.
pub mod header;

/// A versioned beacon block.
pub mod blinded_block;

/// The signed proposal
pub mod beacon_block;

/// The KZG commitments as per EIP-4844
pub mod kzg_commitments;

/// An enum to represent the different forks.
pub mod fork;
/// An helper structure to store versioned data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versioned<T> {
    /// The version.
    pub version: ForkName,
    /// The data for the base fork, now generic.
    pub data: T,
}

impl<T> Versioned<T> {
    /// Create a new instance of the versioned entity with generic data.
    pub fn new(version: ForkName, data: T) -> Self {
        Self { version, data }
    }
}

/// This structs represents the Deneb fork execution payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlindedBlockResponse {
    /// The execution payload.
    pub execution_payload: Arc<ExecutionPayload>,
    /// The blobs bundle.
    pub blobs_bundle: Arc<BlobsBundle>,
}

impl BlindedBlockResponse {
    /// Returns the execution payload.
    pub fn execution_payload(&self) -> Arc<ExecutionPayload> {
        self.execution_payload.clone()
    }

    /// Returns the blobs bundle.
    pub fn blobs_bundle(&self) -> Arc<BlobsBundle> {
        self.blobs_bundle.clone()
    }
}
