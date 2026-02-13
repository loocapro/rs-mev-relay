use crate::server::with_storage;
use crate::server::WarpResult;
use relay_primitives::storage::bid_submission::BidSubmission;
use relay_storage::Storage;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::Instant;
use tracing::trace;
use warp::reply;
use warp::Filter;
use warp::Reply;

#[derive(Deserialize, Debug)]
struct QueryParams {
    slot: Option<u64>,
    block_number: Option<u64>,
}

/// The error type for the get bids route.
#[derive(Debug, Deserialize, Clone, Serialize, thiserror::Error)]
pub enum GetBidsError {
    /// Missing query params.
    #[error("One of slot or block_number must be provided.")]
    MissingQueryParam,
}

impl warp::reject::Reject for GetBidsError {}

/// Route for GET `/relay/v1/data/bidtraces/builder_blocks_received`
/// Used by builders to find the best bid for a given slot or block number.
///
/// See also <https://flashbots.github.io/relay-specs/#/Data/getReceivedBids>
pub fn get_bids<S: Storage + Clone + Send + Sync>(
    storage: S,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("relay" / "v1" / "data" / "bidtraces" / "builder_blocks_received")
        .and(with_storage(storage))
        .and(warp::query::<QueryParams>())
        .and_then(handler)
}

async fn handler<S: Storage>(storage: S, params: QueryParams) -> WarpResult<impl Reply> {
    let start = Instant::now();
    if let Some(slot) = params.slot {
        if let Some(bid) = storage.read_best_bid_by_slot(slot) {
            let elapsed = start.elapsed();
            trace!(
                ?slot,
                ?elapsed,
                "GET /relay/v1/data/bidtraces/builder_blocks_received"
            );
            return Ok(reply::json(&vec![bid]));
        }
    }

    if let Some(block_number) = params.block_number {
        if let Some(bid) = storage.read_best_bid_by_block_number(block_number) {
            let elapsed = start.elapsed();
            trace!(
                ?block_number,
                ?elapsed,
                "GET /relay/v1/data/bidtraces/builder_blocks_received"
            );
            return Ok(reply::json(&vec![bid]));
        }
    }
    Ok(reply::json::<Vec<BidSubmission>>(&vec![]))
}
