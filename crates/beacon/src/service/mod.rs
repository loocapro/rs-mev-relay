use self::handle::BeaconHandle;
use crate::api::SubmissionType;
use crate::BeaconApi;
use futures::{Future, StreamExt};
use relay_primitives::beacon::known_validator::KnownValidatorResponse;
use relay_primitives::beacon::proposer_duties::ProposerDutiesResponse;
use relay_primitives::beacon::sync_status::BeaconSyncStatusResponse;
use relay_primitives::beacon::{beacon_block::SignedBeaconBlockContent, fork::GetForkResponse};
use relay_primitives::blst::public_key::BlsPublicKey;
use relay_primitives::BoxedFuture;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use thiserror::Error;
use tokio::sync::{
    mpsc::{self, error::SendError},
    oneshot::{self, error::RecvError},
};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::info;
/// Service handle used to send commands to the beacon service.
pub mod handle;

type ForkDataFuture = BoxedFuture<GetForkResponse>;
type SyncStatusFuture = BoxedFuture<BeaconSyncStatusResponse>;
type ProposerDutyFuture = BoxedFuture<ProposerDutiesResponse>;
type KnownValidatorFuture = BoxedFuture<KnownValidatorResponse>;

/// Error type for the beacon service.
#[derive(Debug, Error)]
pub enum BeaconServiceErr {
    /// Error from tokio sync oneshot channel.
    #[error("Could not receive response")]
    BrokenRecvChannel(#[from] RecvError),
    /// Error from tokio sync mpsc channel.
    #[error("Could not send message")]
    BrokenSendChannel(#[from] SendError<BeaconCommands>),
}

/// Commands that can be sent to the beacon service.
pub enum BeaconCommands {
    /// Command used to request the sync status of the beacon node.
    SyncStatus(oneshot::Sender<SyncStatusFuture>),
    /// Command used to request the known validators of the beacon consensus.
    ForkData(oneshot::Sender<ForkDataFuture>),
    /// Command used to request the proposer duties of the beacon node for a given epoch.
    ProposerDuties(u64, oneshot::Sender<ProposerDutyFuture>),
    /// Command used to request a known validator from its pubkey of the beacon consensus.
    KnownValidator(BlsPublicKey, oneshot::Sender<KnownValidatorFuture>),
    /// Command used to publish a block to the beacon node.
    PublishBlock(
        Box<SignedBeaconBlockContent>,
        SubmissionType,
        oneshot::Sender<BoxedFuture<()>>,
    ),
}

/// Service responsible for handling beacon node related requests.
pub struct BeaconService<API: BeaconApi> {
    beacon_api: Arc<API>,
    service_tx: mpsc::UnboundedSender<BeaconCommands>,
    command_rx: UnboundedReceiverStream<BeaconCommands>,
}

impl<API: BeaconApi + 'static> BeaconService<API> {
    /// Creates a new beacon service and its handle.
    pub fn new(beacon_api: Arc<API>) -> (Self, BeaconHandle) {
        let (service_tx, command_rx) = mpsc::unbounded_channel();
        let service = Self {
            beacon_api,
            command_rx: UnboundedReceiverStream::new(command_rx),
            service_tx,
        };
        let handle = service.handle();
        info!("Beacon service started.");
        (service, handle)
    }

    /// Returns a handle to the beacon service.
    fn handle(&self) -> BeaconHandle {
        BeaconHandle {
            to_service: self.service_tx.clone(),
        }
    }
    /// Wraps the sync status method of the beacon node api into a boxed future.
    fn sync_status(&self) -> SyncStatusFuture {
        let api = self.beacon_api.clone();
        Box::pin(async move { api.sync_status().await })
    }

    fn get_fork_data(&self) -> ForkDataFuture {
        let api = self.beacon_api.clone();
        Box::pin(async move { api.get_fork_data().await })
    }
    fn get_known_validator(&self, pubkey: BlsPublicKey) -> KnownValidatorFuture {
        let api = self.beacon_api.clone();
        Box::pin(async move { api.get_known_validator(pubkey).await })
    }
    /// Wraps the proposer duties method of the beacon node api into a boxed future.
    fn proposer_duties(&self, epoch: u64) -> ProposerDutyFuture {
        let api = self.beacon_api.clone();
        Box::pin(async move { api.proposer_duties(epoch).await })
    }

    fn publish_block(
        &self,
        block: SignedBeaconBlockContent,
        submission_type: SubmissionType,
    ) -> BoxedFuture<()> {
        let api = self.beacon_api.clone();
        Box::pin(async move { api.publish_block(block, submission_type).await })
    }
}

impl<API: BeaconApi + 'static> Future for BeaconService<API> {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        while let Poll::Ready(Some(command)) = self.command_rx.poll_next_unpin(cx) {
            match command {
                BeaconCommands::SyncStatus(tx) => {
                    let fut = self.sync_status();
                    let _ = tx.send(fut);
                }
                BeaconCommands::KnownValidator(pubkey, tx) => {
                    let fut = self.get_known_validator(pubkey);
                    let _ = tx.send(fut);
                }
                BeaconCommands::ForkData(tx) => {
                    let fut = self.get_fork_data();
                    let _ = tx.send(fut);
                }
                BeaconCommands::ProposerDuties(epoch, tx) => {
                    let fut = self.proposer_duties(epoch);
                    let _ = tx.send(fut);
                }
                BeaconCommands::PublishBlock(block, submission_type, tx) => {
                    let fut = self.publish_block(*block, submission_type);
                    let _ = tx.send(fut);
                }
            }
        }
        Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use relay_primitives::{
        beacon::{known_validator::KnownValidatorResponse, sync_status::BeaconSyncStatus},
        blst::public_key::BlsPublicKey,
    };
    use url::Url;

    fn proposer_duties_response() -> ProposerDutiesResponse {
        let multiple_duties_json = r#"{
            "data": [
                {
                    "slot": "123456",
                    "pubkey": "{{TEST_BLS_PUBKEY}}",
                    "validator_index": "654321"
                },
                {
                    "slot": "123457",
                    "pubkey": "{{TEST_BLS_PUBKEY}}",
                    "validator_index": "654322"
                },
                {
                    "slot": "123458",
                    "pubkey": "{{TEST_BLS_PUBKEY}}",
                    "validator_index": "654323"
                }
            ]
        }"#;
        let json = multiple_duties_json.replace(
            "{{TEST_BLS_PUBKEY}}",
            relay_primitives::test_utils::test_bls_pubkey_str(),
        );
        serde_json::from_str(&json).unwrap()
    }

    struct TestApi;

    impl BeaconApi for TestApi {
        type Config = Url;
        fn new(_url: Url) -> Self {
            TestApi
        }
        async fn get_fork_data(&self) -> eyre::Result<GetForkResponse> {
            unimplemented!()
        }
        async fn get_known_validator(
            &self,
            _pubkey: BlsPublicKey,
        ) -> eyre::Result<KnownValidatorResponse> {
            unimplemented!()
        }
        async fn publish_block(
            &self,
            _block: SignedBeaconBlockContent,
            _submission_type: SubmissionType,
        ) -> eyre::Result<()> {
            Ok(())
        }
        async fn sync_status(&self) -> eyre::Result<BeaconSyncStatusResponse> {
            Ok(BeaconSyncStatusResponse {
                data: BeaconSyncStatus {
                    is_syncing: false,
                    head_slot: 0,
                },
            })
        }

        async fn proposer_duties(&self, _epoch: u64) -> eyre::Result<ProposerDutiesResponse> {
            Ok(proposer_duties_response())
        }
    }

    #[tokio::test]
    async fn can_send_commands() {
        let test_api = TestApi::new(Url::parse("http://something").unwrap());
        let (service, handle) = BeaconService::<TestApi>::new(test_api.into());

        tokio::spawn(async move {
            service.await;
        });

        let response = handle.sync_status().await;
        assert!(response.is_ok());
        let response = handle.proposer_duties(0).await;
        assert!(response.is_ok());
    }
}
