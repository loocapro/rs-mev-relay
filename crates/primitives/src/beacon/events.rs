use futures::Stream;
use mev_share_sse::client::SseError;
use reth::rpc::types::beacon::events::HeadEvent;
use reth::rpc::types::beacon::events::PayloadAttributesEvent;
use std::pin::Pin;

/// A stream of events.
pub type EventStream<T> = Pin<Box<dyn Stream<Item = Result<T, SseError>> + Send>>;

/// The different types of beacon events we subscribe to
#[derive(Debug, Eq, PartialEq)]
pub enum BeaconEvent {
    /// A payload attributes event.
    PayloadAttribute(PayloadAttributesEvent),
    /// A new head event.
    Head(HeadEvent),
}

/// Represents the different topics that can be subscribed to.
pub enum BeaconEventTopic {
    /// Subscription topic for payload attributes events.
    PayloadAttributes,
    /// Subscription topic for new head events.
    NewHead,
    /// Subscription topic for new block events.
    NewBlock,
}

impl BeaconEventTopic {
    /// Returns the string representation of the topic.
    pub fn as_str(&self) -> &'static str {
        match self {
            BeaconEventTopic::PayloadAttributes => "payload_attributes",
            BeaconEventTopic::NewHead => "head",
            BeaconEventTopic::NewBlock => "block",
        }
    }
}
