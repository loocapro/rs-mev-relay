use crate::server::with_json_body;
use crate::server::with_storage;
use crate::server::WarpResult;
use relay_primitives::storage::bid_submission::BidSubmission;
use relay_primitives::storage::bid_traces::BidTrace;
use relay_storage::Storage;
use serde::Deserialize;
use serde::Serialize;
use std::sync::Arc;
use tracing::error;
use tracing::warn;
use warp::reject::Reject;
use warp::Filter;
use warp::Reply;

/// The error type for the post blinded block route.
#[derive(Debug, thiserror::Error, Serialize, Clone, Deserialize)]
pub enum PostBidsError {
    /// The bid value is below the best bid.
    #[error("Bid value is below floor.")]
    BelowFloor,
    /// The bid value is zero.
    #[error("Bid value 0.")]
    ZeroBid,
    /// The builder is not a whitelisted builder.
    #[error("Builder is not allowed to submit bids.")]
    UnauthorizedBuilder,
    /// The slot is in the past.
    #[error("Submission is for past slot.")]
    PastSlotSubmission,
    /// No duty found from slot.
    #[error("No duty found from slot.")]
    DutyNotFound,

    /// Invalid signature.
    #[error("Invalid signature.")]
    InvalidSignature,
    /// Invalid payload attributes.
    #[error("Invalid payload attributes: {0}.")]
    InvalidPayloadAttrs(String),
}

impl Reject for PostBidsError {}

/// Route for POST `/relay/v1/builder/blocks`
///
/// See also <https://flashbots.github.io/relay-specs/#/Builder/submitBlock>
pub fn post_bids<S: Storage + Clone + Send + Sync>(
    storage: S,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let payload_limit = 1024 * 1024 * 8; // 8mb
    warp::path!("relay" / "v1" / "builder" / "blocks")
        .and(warp::post())
        .and(with_storage(storage))
        .and(with_json_body::<BidSubmission>(payload_limit))
        .and_then(handler)
}

async fn handler<S: Storage + Clone>(
    storage: S,
    submission: BidSubmission,
) -> WarpResult<impl Reply> {
    let bid = submission.bid_trace();

    if !is_bid_higher(storage.clone(), bid) {
        warn!("Incoming bid is below or equal to the current best bid");
        return Err(warp::reject::custom(PostBidsError::BelowFloor));
    }

    if bid.value.as_ref().is_zero() {
        error!("Bid value 0");
        return Err(warp::reject::custom(PostBidsError::ZeroBid));
    }

    if !storage.is_whitelisted_builder(&bid.builder_pubkey) {
        let builder = bid.builder_pubkey;
        error!(?builder, "Unauthorized Builder");
        return Err(warp::reject::custom(PostBidsError::UnauthorizedBuilder));
    }

    let head_slot = storage.read_head_slot();
    if !head_slot.is_next_slot(bid.slot) {
        let slot = bid.slot;
        error!(?head_slot, slot, "Submission for past slot");
        return Err(warp::reject::custom(PostBidsError::PastSlotSubmission));
    }
    let execution_payload = submission.execution_payload();

    let attrs = storage.read_payload_attributes();
    if let Err(error_message) = attrs.validate_bid(bid.slot, execution_payload.prev_randao) {
        error!("{:?}", &error_message);
        return Err(warp::reject::custom(PostBidsError::InvalidPayloadAttrs(
            error_message.to_string(),
        )));
    }

    let duty = match storage.find_duty_by_slot(bid.slot) {
        Some(duty) => duty,
        None => {
            error!("No duty found");
            return Err(warp::reject::custom(PostBidsError::DutyNotFound));
        }
    };

    let builder_domain = storage.fork_data().compute_builder_domain();
    let is_verified = bid.verify_signature(submission.signature(), builder_domain);

    if !is_verified {
        let json = serde_json::to_string(bid).unwrap();
        let sig = submission.signature();
        error!(?json, ?builder_domain, ?sig, "Invalid signature");
        return Err(warp::reject::custom(PostBidsError::InvalidSignature));
    }

    let slot = bid.slot;
    let submission = BidSubmission::new(
        bid.clone(),
        execution_payload,
        submission.blobs_bundle(),
        submission.signature().clone(),
    );
    let blinded_block_response = submission.to_blinded_block_response();

    storage.set_best_bid(slot, Arc::new(submission));
    storage.set_blinded_block_response(duty.entry.message.pubkey, blinded_block_response);

    Ok::<_, warp::Rejection>(warp::reply::with_status(
        "OK",
        warp::http::status::StatusCode::OK,
    ))
}

fn is_bid_higher<S: Storage>(storage: S, bid: &BidTrace) -> bool {
    let slot = bid.slot;
    let parent_hash = bid.parent_hash;
    let proposer = bid.proposer_pubkey;
    if let Some(current_best) = storage.read_best_bid(slot, parent_hash, proposer) {
        return bid > current_best.bid_trace();
    }
    true
}

#[cfg(test)]
mod tests {
    use relay_primitives::storage::bid_submission::BidSubmission;

    #[tokio::test]
    async fn can_bid() {
        bid();
    }

    fn bid() -> BidSubmission {
        const PLACEHOLDER: &str = "{{TEST_BUILDER_PUBKEY}}";
        let json_string = r#"
        {
  "message": {
    "slot": "1",
    "parent_hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "block_hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "builder_pubkey": "{{TEST_BUILDER_PUBKEY}}",
    "proposer_pubkey": "{{TEST_BUILDER_PUBKEY}}",
    "proposer_fee_recipient": "0x0000000000000000000000000000000000000001",
    "gas_limit": "1",
    "gas_used": "1",
    "value": "1"
  },
  "execution_payload": {
    "parent_hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "fee_recipient": "0x0000000000000000000000000000000000000001",
    "state_root": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "receipts_root": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "logs_bloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "prev_randao": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "block_number": "1",
    "gas_limit": "1",
    "gas_used": "1",
    "timestamp": "1",
    "extra_data": "0x7465737400000000000000000000000000000000000000000000000000000000",
    "base_fee_per_gas": "1",
    "blob_gas_used": "1",
    "excess_blob_gas": "1",
    "block_hash": "0x0000000000000000000000000000000000000000000000000000000000000001",
    "transactions": [
      "0x02f87880648252089400000000000000000000000000000000000000018080c001a00000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000"
    ],
    "withdrawals": [
      {
        "index": "1",
        "validator_index": "1",
        "address": "0x0000000000000000000000000000000000000001",
        "amount": "32000000000"
      }
    ]
  },
  "blobs_bundle": {
    "commitments": [
      "0x8dab030c51e16e84be9caab84ee3d0b8bbec1db4a0e4de76439da8424d9b957370a10a78851f97e4b54d2ce1ab0d686f"
    ],
    "proofs": [
      "0xb4021b0de10f743893d4f71e1bf830c019e832958efd6795baf2f83b8699a9eccc5dc99015d8d4d8ec370d0cc333c06a"
    ],
    "blobs": []
  },
  "signature": "0xad92d76a40ecc96a5ab46ae0a7e62c50fc895412be3ae3e0cddad60c54be7aa2c4bbe90b40345e3c2d1a6925aa3a9c6e03f4e30e28b9a315cac4d73de4b8f77a77e0a4f63fa7d6e0a73393cb15cfe596d845eaa146fd52cd9f3b80cde3f2b27a"
}
        "#;
        let json_string = json_string.replace(
            PLACEHOLDER,
            relay_primitives::test_utils::test_builder_pubkey_str(),
        );
        serde_json::from_str(&json_string).unwrap()
    }
}
