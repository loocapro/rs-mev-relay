use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::DisplayFromStr;

/// Represents the synchronization status of a beacon node, fetched from `/eth/v1/node/syncing`.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct BeaconSyncStatusResponse {
    /// Contains detailed sync status information.
    pub data: BeaconSyncStatus,
}

/// Detailed synchronization status of a beacon node.
#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
pub struct BeaconSyncStatus {
    /// The highest slot that the node has fully processed and is in sync with.
    #[serde_as(as = "DisplayFromStr")]
    pub head_slot: u64,
    /// Indicates whether the node is currently catching up to the latest chain head.
    pub is_syncing: bool,
}
