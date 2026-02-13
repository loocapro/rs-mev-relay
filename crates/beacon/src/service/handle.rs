use crate::api::SubmissionType;

use super::BeaconCommands;
use relay_primitives::{
    beacon::{
        beacon_block::SignedBeaconBlockContent, fork::GetForkResponse,
        known_validator::KnownValidatorResponse, proposer_duties::ProposerDutiesResponse,
        sync_status::BeaconSyncStatusResponse,
    },
    blst::public_key::BlsPublicKey,
};

/// Service handle used to send commands to the beacon service.
#[derive(Debug, Clone)]
pub struct BeaconHandle {
    /// Channel used to send commands to the beacon service.
    pub(crate) to_service: tokio::sync::mpsc::UnboundedSender<BeaconCommands>,
}

impl BeaconHandle {
    /// Sends a command to the beacon service to request the sync status of the beacon node.
    pub async fn sync_status(&self) -> eyre::Result<BeaconSyncStatusResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.to_service.send(BeaconCommands::SyncStatus(tx))?;
        rx.await?.await
    }

    /// Sends a command to the beacon service to request the fork data of the beacon node.
    pub async fn get_fork_data(&self) -> eyre::Result<GetForkResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.to_service.send(BeaconCommands::ForkData(tx))?;
        rx.await?.await
    }

    /// Sends a command to the beacon service to request a known validator from its pubkey of the beacon consensus.
    pub async fn get_known_validator(
        &self,
        pubkey: BlsPublicKey,
    ) -> eyre::Result<KnownValidatorResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.to_service
            .send(BeaconCommands::KnownValidator(pubkey, tx))?;

        rx.await?.await
    }

    /// Sends a command to the beacon service to request the proposer duties of the beacon node for a given epoch.
    pub async fn proposer_duties(&self, epoch: u64) -> eyre::Result<ProposerDutiesResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.to_service
            .send(BeaconCommands::ProposerDuties(epoch, tx))?;
        rx.await?.await
    }

    /// Sends a command to the beacon service to publish a block to the beacon node.
    pub async fn publish_block(
        &self,
        block: SignedBeaconBlockContent,
        submission_type: SubmissionType,
    ) -> eyre::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.to_service.send(BeaconCommands::PublishBlock(
            Box::new(block),
            submission_type,
            tx,
        ))?;
        rx.await?.await
    }
}
