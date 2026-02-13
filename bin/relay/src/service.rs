use beacon::service::handle::BeaconHandle;
use beacon::{events::stream::BeaconEventsStream, BeaconConnection};
use futures_util::StreamExt;
use relay_primitives::beacon::events::BeaconEvent;
use relay_primitives::beacon::proposer_duties::ProposerDutiesData;
use relay_primitives::blst::public_key::BlsPublicKey;
use relay_primitives::storage::head_slot::HeadSlot;
use relay_primitives::storage::payload_attributes::PayloadAttributes;
use relay_primitives::storage::validator::Validator;
use relay_storage::Storage;
use std::sync::Arc;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tracing::log::error;
use tracing::{info, warn};

/// The backoff time to wait before reconnecting to the beacon events stream.
const BACKOFF: std::time::Duration = std::time::Duration::from_secs(1);

type Connection = Pin<Box<dyn Future<Output = BeaconEventsStream> + Send + 'static>>;

/// The inner state of the relay service used to handle reconnections
enum Inner {
    /// The beacon events stream
    BeaconEvents(BeaconEventsStream),
    /// The future to reconnect to the beacon events stream
    PendingConnection(Connection),
}

/// The relay service is responsible for listening to beacon events and updating the storage accordingly.
pub(crate) struct RelayService<S: Storage, C: BeaconConnection> {
    /// The inner state of the relay service used to handle reconnections
    inner: Inner,
    /// The relay global storage
    storage: S,
    /// The beacon events to listen to
    beacon_events: Arc<C>,
    beacon_handle: BeaconHandle,
    slots_per_epoch: u64,
}

impl<S, C> RelayService<S, C>
where
    S: Storage,
    C: BeaconConnection,
{
    /// Creates a new instance of the relay service
    pub(crate) async fn new(
        beacon_events: Arc<C>,
        storage: S,
        beacon_handle: BeaconHandle,
        slots_per_epoch: u64,
    ) -> Self {
        Self {
            inner: Inner::BeaconEvents(beacon_events.stream().await),
            storage,
            beacon_events: beacon_events.clone(),
            beacon_handle,
            slots_per_epoch,
        }
    }
}

/// Helper function to format the duties into a storable format
/// If there are no validators registered in the storage, it will return an empty vector.
pub(crate) fn into_duties<S: Storage>(storage: S, duties: &[ProposerDutiesData]) -> Vec<Validator> {
    if storage.empty_validator_regs() {
        return Vec::new();
    }

    duties
        .iter()
        .filter_map(|duty| {
            let pubkey = BlsPublicKey::from(duty.pubkey);
            storage
                .read_validator_registration(&pubkey)
                .map(|registration| Validator {
                    entry: registration,
                    validator_index: duty.validator_index,
                    slot: duty.slot,
                })
        })
        .collect()
}

impl<S: Storage + Clone + Unpin + 'static + Send, C: BeaconConnection + 'static> Future
    for RelayService<S, C>
{
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &mut self.as_mut().get_mut().inner {
            Inner::BeaconEvents(beacon_stream) => match beacon_stream.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => {
                    match event {
                        BeaconEvent::PayloadAttribute(payload_attribute) => {
                            let p_attr = PayloadAttributes::from(payload_attribute.data);

                            let slot = p_attr.proposal_slot();
                            let parent = p_attr.parent_block_number();

                            let fee_recipient = p_attr.suggested_fee_recipient();
                            info!(
                                ?slot,
                                ?parent,
                                ?fee_recipient,
                                "New payload attribute event"
                            );
                            self.storage.set_payload_attributes(p_attr);
                        }
                        BeaconEvent::Head(head) => {
                            let slot = head.slot;

                            info!(?slot, "New head slot event");
                            let head_slot = HeadSlot::from(slot);
                            self.storage.set_head_slot(head_slot);

                            // Only process every 32 slots
                            if slot % 32 == 0 {
                                let storage = self.storage.clone();
                                let beacon_handle = self.beacon_handle.clone();
                                let slots_per_epoch = self.slots_per_epoch;
                                tokio::task::spawn(async move {
                                    // wait until the 6th seconds of the slot to reduce pressure on prysm
                                    tokio::time::sleep(tokio::time::Duration::from_secs(6)).await;

                                    let started = std::time::Instant::now();

                                    let epoch = head_slot.epoch(slots_per_epoch);
                                    let duties_epoch = beacon_handle.proposer_duties(epoch);
                                    let duties_next_epoch =
                                        beacon_handle.proposer_duties(epoch + 1);
                                    let (duties_epoch_res, duties_next_epoch_res) =
                                        tokio::join!(duties_epoch, duties_next_epoch);
                                    let validators =
                                        duties_epoch_res?.merge_with(duties_next_epoch_res?);
                                    let duties = into_duties(storage.clone(), &validators);
                                    if !duties.is_empty() {
                                        storage.set_proposer_duties(duties);
                                    } else {
                                        warn!("No signed registrations found for the coming epoch. Skipping duties.");
                                    }
                                    let elapsed = started.elapsed();

                                    info!(
                                        ?elapsed,
                                        "Updated known validators and duties after 6s delay."
                                    );
                                    Ok::<(), eyre::Report>(())
                                });
                            }
                        }
                    }
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Poll::Ready(None) => {
                    error!("Beacon connection ended, will keep retrying to connect.");
                    let beacon_events_clone = self.beacon_events.clone();
                    let reconnect_future = Box::pin(async move {
                        tokio::time::sleep(BACKOFF).await;
                        beacon_events_clone.stream().await
                    });
                    self.as_mut().get_mut().inner = Inner::PendingConnection(reconnect_future);
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Poll::Pending => Poll::Pending,
            },
            Inner::PendingConnection(conn_fut) => match conn_fut.as_mut().poll(cx) {
                Poll::Ready(beacon_stream) => {
                    info!("Beacon connection re-established");
                    self.as_mut().get_mut().inner = Inner::BeaconEvents(beacon_stream);
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Poll::Pending => Poll::Pending,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::service::{BeaconConnection, RelayService};
    use beacon::{
        api::SubmissionType, events::stream::BeaconEventsStream, service::BeaconService, BeaconApi,
    };
    use futures_util::stream;
    use relay_primitives::{
        beacon::{
            beacon_block::SignedBeaconBlockContent, fork::GetForkResponse,
            known_validator::KnownValidatorResponse, proposer_duties::ProposerDutiesResponse,
            sync_status::BeaconSyncStatusResponse,
        },
        blst::{public_key::BlsPublicKey, signer::ForkDatas},
        events::{HeadEvent, PayloadAttributesEvent},
        revm_primitives::{fixed_bytes, HashMap},
    };
    use relay_storage::{in_memory::InMemoryStorage, Storage};

    #[tokio::test]
    async fn can_fill_storage_from_events_through_service() {
        let storage = InMemoryStorage::new(
            0.into(),
            payload_attribute_event().data.into(),
            vec![],
            ForkDatas::default(),
            HashMap::new(),
            vec![],
        );

        let api = Arc::new(TestApi::new(()));
        let (beacon_service, beacon_handle) = BeaconService::new(api);
        tokio::spawn(async move {
            beacon_service.await;
        });

        let relay_service = RelayService::new(
            MockBeaconConnection.into(),
            storage.clone(),
            beacon_handle,
            32,
        )
        .await;
        tokio::spawn(relay_service);
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let validators = storage.read_proposer_duties();
        let slot = storage.read_head_slot();
        let payload_attr = storage.read_payload_attributes();
        // no registrations yet so we should not have any duties
        assert_eq!(validators.len(), 0);
        assert_eq!(slot, 1.into());
        assert_eq!(payload_attr, payload_attribute_event().data.into());
    }

    #[derive(Clone)]
    struct TestApi;

    impl BeaconApi for TestApi {
        type Config = ();

        fn new(_config: Self::Config) -> Self {
            TestApi
        }
        async fn get_known_validator(
            &self,
            _pubkey: BlsPublicKey,
        ) -> eyre::Result<KnownValidatorResponse> {
            unimplemented!()
        }
        async fn get_fork_data(&self) -> eyre::Result<GetForkResponse> {
            unimplemented!()
        }
        async fn sync_status(&self) -> eyre::Result<BeaconSyncStatusResponse> {
            unimplemented!()
        }

        async fn proposer_duties(&self, _epoch: u64) -> eyre::Result<ProposerDutiesResponse> {
            Ok(proposer_duties_response())
        }

        async fn publish_block(
            &self,
            _block: SignedBeaconBlockContent,
            _submission_type: SubmissionType,
        ) -> eyre::Result<()> {
            Ok(())
        }
    }

    struct MockBeaconConnection;

    impl BeaconConnection for MockBeaconConnection {
        fn stream(
            &self,
        ) -> impl futures_util::Future<Output = BeaconEventsStream> + std::marker::Send {
            let payload_events = stream::iter(vec![Ok(payload_attribute_event())]);
            let head_events = stream::iter(vec![Ok(head_event(1))]);

            Box::pin(async move {
                BeaconEventsStream::new(Box::pin(payload_events), Box::pin(head_events))
            })
        }
    }

    fn payload_attribute_event() -> PayloadAttributesEvent {
        let json = r#"
    {
      "version": "capella",
      "data": {
        "proposal_slot": "7",
        "parent_block_root": "0xc7b8655cfbe8522f2dcd2acef7bb32259f689b8b29605c039cae66447dc339a6",
        "parent_block_number": "6",
        "parent_block_hash": "0x007bf462285673e400338ccd62647e3bed5eb4fb1201c66878230fea1db082fd",
        "proposer_index": "7",
        "payload_attributes": {
          "timestamp": "1709293522",
          "prev_randao": "0x336bd018d8def745764831b852953d5debb1d86fa4224374be51c21ce426ed45",
          "suggested_fee_recipient": "0x123463a4b065722e99115d6c222f267d9cabb524",
          "withdrawals": [],
          "parent_beacon_block_root": null
        }
      }
    }
    "#;
        serde_json::from_str(json).unwrap()
    }

    fn head_event(slot: u64) -> HeadEvent {
        HeadEvent {
            slot,
            block: fixed_bytes!("1217930f241535c1757cf5245b48a7836ef7ee965e724de94ec1a8d224fbfafe"),
            state: fixed_bytes!("46982bbe2ca198f7971165be649c3fa9a9d7673ccfc17214ff0f5a03f5a7c641"),
            epoch_transition: false,
            previous_duty_dependent_root: fixed_bytes!(
                "dba510ac6871bf5c44721132f5a9de02e6658c64949926b1c4f9cab4841cd76f"
            ),
            current_duty_dependent_root: fixed_bytes!(
                "dba510ac6871bf5c44721132f5a9de02e6658c64949926b1c4f9cab4841cd76f"
            ),
            execution_optimistic: false,
        }
    }

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
}
