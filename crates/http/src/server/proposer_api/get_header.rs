use crate::server::with_signer;

use crate::server::with_storage;
use crate::server::WarpResult;
use relay_primitives::beacon::fork::ForkName;
use relay_primitives::beacon::header::SignedHeader;

use relay_primitives::beacon::Versioned;
use relay_primitives::blst::public_key::BlsPublicKey;

use relay_primitives::blst::signer::BlsSigner;
use relay_primitives::hex;
use relay_primitives::metrics::RelayMetrics;
use relay_primitives::types::B256;
use relay_storage::Storage;
use serde::Deserialize;
use serde::Serialize;
use tokio::time::Instant;
use tracing::error;
use tracing::info;
use warp::reject::Reject;
use warp::reply;
use warp::Filter;
use warp::Reply;

/// Route for GET `/eth/v1/builder/header/{slot}/{parent_hash}/{pubkey}`
///
/// See also <https://ethereum.github.io/builder-specs/#/Builder/getHeader>
pub fn get_header<S: Storage + Clone + Send + Sync>(
    storage: S,
    signer: BlsSigner,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("eth" / "v1" / "builder" / "header" / u64 / B256 / BlsPublicKey)
        .and(with_storage(storage))
        .and(with_signer(signer))
        .and_then(handler)
}

/// Errors thrown by the get_header route.
#[derive(Debug, thiserror::Error, Serialize, Clone, Deserialize)]
pub enum GetHeaderError {
    /// The slot sent is too old compared to the current slot.
    #[error("slot is too old")]
    InvalidSlot,
    /// No bid found from the slot, parent_hash and proposer pubkey.
    #[error("No bid found")]
    NoBidFound,
    /// The validator is not authorized to get the header.
    #[error("Not authorized to get the header.")]
    UnauthorizedGetHeader,
}
impl Reject for GetHeaderError {}

async fn handler<S: Storage>(
    slot: u64,
    parent_hash: B256,
    pubkey: BlsPublicKey,
    storage: S,
    signer: BlsSigner,
) -> WarpResult<impl Reply> {
    // we validate the MBS key on validator registration
    // so we expect that if the validator is inside the storage
    // it is authorized to interact with us
    if storage.read_validator_registration(&pubkey).is_none() {
        error!(?pubkey, "Unauthorized validator to get header.");
        return Err(warp::reject::custom(GetHeaderError::UnauthorizedGetHeader));
    }
    let current_slot = storage.read_head_slot();
    let started = Instant::now();

    if !current_slot.is_next_slot(slot) {
        error!(
            ?slot,
            ?parent_hash,
            ?current_slot,
            ?pubkey,
            "slot is too old"
        );
        return Err(warp::reject::custom(GetHeaderError::InvalidSlot));
    }

    let bid = storage
        .read_best_bid(slot, parent_hash, pubkey)
        .ok_or_else(|| {
            error!(?slot, ?parent_hash, ?pubkey, "No bid found");
            warp::reject::custom(GetHeaderError::NoBidFound)
        })?;

    let bid = bid.as_ref().clone();
    let value = bid.bid_trace().value;
    let tx_count = bid.execution_payload().transactions.len();
    let versioned_header =
        Versioned::<SignedHeader>::new(ForkName::Deneb, SignedHeader::try_from((bid, signer))?);

    let elapsed = started.elapsed();
    let extra_data = &versioned_header.data.message.header.extra_data;
    let extra_data = hex::encode(extra_data.to_vec());

    let metrics = RelayMetrics::default();
    metrics.relay.inc_headers();
    if let Ok(value) = TryInto::<u64>::try_into(value.as_ref()) {
        metrics.relay.set_header_values(value);
    }
    metrics.relay.set_tx_count(tx_count);
    metrics.relay.set_header_latency(elapsed.as_millis() as u64);

    info!(
         target: "proposer_api::get_header",
        ?slot,
        ?parent_hash,
        ?pubkey,
        ?current_slot,
        ?extra_data,
        ?elapsed,
        "GET /eth/v1/builder/header/{slot}/{parent_hash}/{pubkey}"
    );

    Ok(reply::json(&versioned_header))
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;
    use std::sync::Arc;

    use crate::server::errors::handle_rejection;
    use crate::server::proposer_api::get_header;
    use relay_primitives::beacon::header::SignedHeader;
    use relay_primitives::beacon::registrations::{
        ValidatorRegistration, ValidatorRegistrationMessage,
    };
    use relay_primitives::blst::public_key::BlsPublicKey;
    use relay_primitives::blst::signature::BlsSignature;
    use relay_primitives::blst::signer::{BlsSigner, ForkDatas};
    use relay_primitives::blst::SignedRoot;
    use relay_primitives::events::PayloadAttributesData;

    use relay_primitives::revm_primitives::HashMap;
    use relay_primitives::storage::bid_submission::BidSubmission;
    use relay_primitives::storage::bid_traces::BidTrace;
    use relay_primitives::types::{Address, BlobsBundle, ExecutionPayload, B256, U256};
    use relay_primitives::validator::PayloadAttributes;

    use relay_storage::in_memory::InMemoryStorage;
    use relay_storage::Storage;
    use ssz_types::{FixedVector, VariableList};
    use warp::http::StatusCode;
    use warp::test::request;
    use warp::Filter;

    #[tokio::test]
    async fn valid_get_header() {
        let storage = setup_storage();
        storage.set_validator_registration(*relay_primitives::test_utils::test_bls_pubkey(), validator_registration(1));

        storage.set_best_bid(11, Arc::new(bid_submission()));
        let signer = BlsSigner::default();
        let pub_key = signer.public_key();
        let domain = signer.fork_data_builder().compute_builder_domain();
        let get_payload = get_header(storage, signer);

        let resp = request()
            .method("GET")
            .path(&format!("/eth/v1/builder/header/11/0x0000000000000000000000000000000000000000000000000000000000000000/{}", relay_primitives::test_utils::test_bls_pubkey_str()))
            .reply(&get_payload)
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.body();
        let versioned_bid: relay_primitives::beacon::Versioned<SignedHeader> =
            serde_json::from_slice(body).unwrap();
        assert_eq!(
            versioned_bid.version,
            relay_primitives::beacon::fork::ForkName::Deneb
        );
        let sig = versioned_bid.data.signature;
        let msg = versioned_bid.data.message;

        assert!(sig.verify(&pub_key, msg.signing_root(domain.into())));
    }

    fn validator_registration(timestamp: u64) -> ValidatorRegistration {
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();

        ValidatorRegistration {
            signature,
            message: ValidatorRegistrationMessage {
                fee_recipient: Address::zero(),
                gas_limit: 0,
                timestamp,
                pubkey: BlsPublicKey::random(),
            },
        }
    }
    #[tokio::test]
    async fn test_invalid_slot_error() {
        let storage = setup_storage();
        storage.set_validator_registration(*relay_primitives::test_utils::test_bls_pubkey(), validator_registration(1));
        let get_payload =
            get_header(storage.clone(), BlsSigner::default()).recover(handle_rejection);
        let path = format!("/eth/v1/builder/header/1/0x0000000000000000000000000000000000000000000000000000000000000000/{}", relay_primitives::test_utils::test_bls_pubkey_str());
        let resp = request()
            .method("GET")
            .path(&path)
            .reply(&get_payload)
            .await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let msg = resp.body();
        assert_eq!(msg, "\"slot is too old\"");
    }

    #[tokio::test]
    async fn test_no_bid_found_error() {
        let storage = setup_storage();
        storage.set_validator_registration(*relay_primitives::test_utils::test_bls_pubkey(), validator_registration(1));
        storage.set_best_bid(10, Arc::new(bid_submission()));
        let get_payload =
            get_header(storage.clone(), BlsSigner::default()).recover(handle_rejection);

        let path = format!("/eth/v1/builder/header/11/0x0000000000000000000000000000000000000000000000000000000000000000/{}", relay_primitives::test_utils::test_bls_pubkey_str());
        let resp = request().method("GET").path(&path).reply(&get_payload).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let msg = resp.body();
        assert_eq!(msg, "\"No bid found\"");
    }

    fn payload_attr(slot: u64, suggested_fee_recipient: Address) -> PayloadAttributesData {
        PayloadAttributesData {
            proposal_slot: slot,
            parent_block_root: relay_primitives::B256::ZERO,
            parent_block_number: 0,
            parent_block_hash: relay_primitives::B256::ZERO,
            proposer_index: 0,
            payload_attributes: PayloadAttributes {
                timestamp: 0,
                prev_randao: relay_primitives::B256::ZERO,
                suggested_fee_recipient: *suggested_fee_recipient.as_ref(),
                withdrawals: Some(vec![]),
                parent_beacon_block_root: None,
            },
        }
    }
    fn setup_storage() -> InMemoryStorage {
        InMemoryStorage::new(
            10.into(),
            payload_attr(0, Address::random()).into(),
            vec![],
            ForkDatas::default(),
            HashMap::new(),
            vec![],
        )
    }

    fn bid_trace(slot: u64, value: String) -> BidTrace {
        let pubkey = *relay_primitives::test_utils::test_bls_pubkey();

        BidTrace {
            slot,
            parent_hash: B256::zero(),
            block_hash: B256::zero(),
            builder_pubkey: pubkey,
            proposer_pubkey: pubkey,
            proposer_fee_recipient: Address::zero(),
            gas_limit: 0,
            gas_used: 0,
            value: U256::from_str(&value).unwrap(),
        }
    }

    fn execution_payload() -> ExecutionPayload {
        ExecutionPayload {
            block_number: 0,
            gas_limit: 0,
            gas_used: 0,
            timestamp: 0,
            extra_data: VariableList::new(vec![]).unwrap(),
            base_fee_per_gas: U256::from(0),
            block_hash: B256::zero(),
            transactions: VariableList::new(vec![]).unwrap(),
            withdrawals: VariableList::new(vec![]).unwrap(),
            blob_gas_used: 0,
            excess_blob_gas: 0,
            parent_hash: B256::zero(),
            prev_randao: B256::zero(),
            fee_recipient: Address::zero(),
            state_root: B256::zero(),
            receipts_root: B256::zero(),
            logs_bloom: FixedVector::new(vec![0; 256]).unwrap(),
        }
    }
    fn bid_submission() -> BidSubmission {
        BidSubmission::new(
            bid_trace(10, "0x0".to_string()),
            Arc::new(execution_payload()),
            Arc::new(BlobsBundle::default()),
            BlsSignature::default(),
        )
    }
}
