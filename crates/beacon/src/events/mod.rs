use self::stream::BeaconEventsStream;
use crate::BeaconConnection;
use relay_primitives::beacon::events::{BeaconEventTopic, EventStream};
use relay_primitives::events::{EventClient, HeadEvent, PayloadAttributesEvent};
use serde::de::DeserializeOwned;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};
use url::Url;
/// The beacon event stream.
pub mod stream;

/// Retry delay in seconds.
const RETRY_DELAY_SECS: u64 = 5;

impl BeaconConnection for BeaconEventsClient {
    async fn stream(&self) -> BeaconEventsStream {
        let payload_attributes = self.payload_attributes().await;
        let new_head = self.new_head().await;
        BeaconEventsStream::new(payload_attributes, new_head)
    }
}

/// Beacon events entry point.
#[derive(Debug)]
pub struct BeaconEventsClient {
    url: Url,
    client: EventClient,
}

impl BeaconEventsClient {
    /// Creates a new `BeaconEventsClient` from the given `EventClient` and `Url`.
    pub fn new(url: &Url) -> Self {
        info!(?url, "Connecting with beacon node this might take a while.",);
        Self {
            client: EventClient::default(),
            url: url.to_owned(),
        }
    }

    /// Subscribes to payload attributes events.
    async fn payload_attributes(&self) -> EventStream<PayloadAttributesEvent> {
        self.subscribe_with_retry(BeaconEventTopic::PayloadAttributes)
            .await
    }

    /// Subscribes to new head events.
    async fn new_head(&self) -> EventStream<HeadEvent> {
        self.subscribe_with_retry(BeaconEventTopic::NewHead).await
    }

    /// Attempts to subscribe to a given event topic and returns an `EventStream`.
    /// Retries indefinitely every 5 seconds upon failure.
    async fn subscribe_with_retry<T: Send + DeserializeOwned + 'static>(
        &self,
        topic: BeaconEventTopic,
    ) -> EventStream<T> {
        let events_url = self.url.clone();
        let url = format!("{}eth/v1/events?topics={}", events_url, topic.as_str());
        loop {
            match self.client.subscribe(&url).await {
                Ok(subscription) => return Box::pin(subscription),
                Err(err) => {
                    warn!(
                        "Failed to subscribe to {} events: {:?}\nRetrying in {} seconds...",
                        topic.as_str(),
                        err,
                        RETRY_DELAY_SECS
                    );
                    sleep(Duration::from_secs(RETRY_DELAY_SECS)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::stream::BeaconEventsStream;
    use crate::api::SubmissionType;
    use crate::BeaconApi;
    use futures::StreamExt;
    use relay_primitives::beacon::beacon_block::SignedBeaconBlockContent;

    use relay_primitives::beacon::fork::GetForkResponse;
    use relay_primitives::beacon::known_validator::KnownValidatorResponse;
    use relay_primitives::blst::public_key::BlsPublicKey;
    use relay_primitives::events::PayloadAttributesEvent;
    use relay_primitives::{
        beacon::{
            events::BeaconEvent, proposer_duties::ProposerDutiesResponse,
            sync_status::BeaconSyncStatusResponse,
        },
        events::{HeadEvent, SseError},
        revm_primitives::fixed_bytes,
    };

    #[tokio::test]
    async fn can_emit_events() {
        let mut stream =
            test_event_stream(vec![Ok(payload_attribute_event())], vec![Ok(head_event(1))]).await;

        assert_next_event(
            &mut stream,
            BeaconEvent::PayloadAttribute(payload_attribute_event()),
        )
        .await;

        assert_next_event(&mut stream, BeaconEvent::Head(head_event(1))).await;
    }

    #[tokio::test]
    async fn no_epoch_event_on_non_first_slot() {
        let mut stream =
            test_event_stream(vec![Ok(payload_attribute_event())], vec![Ok(head_event(2))]).await;

        assert_next_event(
            &mut stream,
            BeaconEvent::PayloadAttribute(payload_attribute_event()),
        )
        .await;

        assert_next_event(&mut stream, BeaconEvent::Head(head_event(2))).await;

        // Attempt to fetch the next event, which we expect to be None,
        // since it's not the first slot of a new epoch and thus should not emit an epoch event
        assert_no_more_events(stream).await;
    }

    #[tokio::test]
    async fn can_handle_errors() {
        let payload_error = Err(SseError::MaxRetriesExceeded(10));
        let head_error = Err(SseError::MaxRetriesExceeded(10));

        let stream = test_event_stream(vec![payload_error], vec![head_error]).await;

        assert_no_more_events(stream).await;
    }

    #[allow(dead_code)]
    struct TestApi;

    impl BeaconApi for TestApi {
        type Config = ();
        fn new(_config: ()) -> Self {
            TestApi
        }
        async fn sync_status(&self) -> eyre::Result<BeaconSyncStatusResponse> {
            unimplemented!()
        }
        async fn get_known_validator(
            &self,
            _pubkey: BlsPublicKey,
        ) -> eyre::Result<KnownValidatorResponse> {
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
        async fn get_fork_data(&self) -> eyre::Result<GetForkResponse> {
            unimplemented!()
        }
    }

    async fn test_event_stream(
        payload_events: Vec<Result<PayloadAttributesEvent, SseError>>,
        head_events: Vec<Result<HeadEvent, SseError>>,
    ) -> BeaconEventsStream {
        BeaconEventsStream::new(
            Box::pin(futures::stream::iter(payload_events)),
            Box::pin(futures::stream::iter(head_events)),
        )
    }

    async fn assert_next_event(stream: &mut BeaconEventsStream, expected_event: BeaconEvent) {
        let event = stream.next().await.unwrap();
        assert_eq!(event, expected_event, "Unexpected event encountered");
    }

    async fn assert_no_more_events(mut stream: BeaconEventsStream) {
        assert!(
            stream.next().await.is_none(),
            "Expected no more events in the stream"
        );
    }

    #[allow(dead_code)]
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
          "suggested_fee_recipient": "0x0000000000000000000000000000000000000001",
          "withdrawals": [],
          "parent_beacon_block_root": null
        }
      }
    }
    "#;
        serde_json::from_str(json).unwrap()
    }
}
