use futures::stream::Stream;
use futures::StreamExt;
use relay_primitives::beacon::events::{BeaconEvent, EventStream};
use relay_primitives::events::{HeadEvent, PayloadAttributesEvent};
use std::pin::Pin;
use std::task::{Context, Poll};
use tracing::error;
use tracing::warn;

/// Helper struct to merge multiple event streams into a single stream.
pub struct BeaconEventsStream {
    payload_attributes: EventStream<PayloadAttributesEvent>,
    new_head: EventStream<HeadEvent>,
}

impl BeaconEventsStream {
    /// Creates a new `BeaconEventsStream` from the given `EventStream`s.
    pub fn new(
        payload_attributes: EventStream<PayloadAttributesEvent>,
        new_head: EventStream<HeadEvent>,
    ) -> Self {
        BeaconEventsStream {
            payload_attributes,
            new_head,
        }
    }
}

impl Stream for BeaconEventsStream {
    type Item = BeaconEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.payload_attributes.as_mut().poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(event))) => {
                return Poll::Ready(Some(BeaconEvent::PayloadAttribute(event)))
            }
            Poll::Ready(Some(Err(err))) => {
                error!("{err:?}");
                return Poll::Ready(None);
            }
            Poll::Ready(None) => {
                warn!("Payload attributes stream ended");
            }
            Poll::Pending => {}
        }

        match self.new_head.as_mut().poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(event))) => {
                return Poll::Ready(Some(BeaconEvent::Head(event)));
            }
            Poll::Ready(Some(Err(err))) => {
                error!("{err:?}");
                return Poll::Ready(None);
            }
            Poll::Ready(None) => {
                error!("Head stream ended");
                return Poll::Ready(None);
            }
            Poll::Pending => {}
        }

        Poll::Pending
    }
}
