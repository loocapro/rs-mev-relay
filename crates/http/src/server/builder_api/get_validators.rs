use crate::server::with_storage;
use crate::server::WarpResult;
use relay_storage::Storage;
use tokio::time::Instant;
use tracing::info;
use warp::reply;
use warp::Filter;
use warp::Reply;

/// Route for GET `/relay/v1/builder/validators`
///
/// See also <https://flashbots.github.io/relay-specs/#/Builder/getValidators>
pub fn get_validators<S: Storage + Clone + Send + Sync>(
    storage: S,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("relay" / "v1" / "builder" / "validators")
        .and(with_storage(storage))
        .and_then(handler)
}

async fn handler<S: Storage>(storage: S) -> WarpResult<impl Reply> {
    let started = Instant::now();
    let duties = storage.read_proposer_duties();
    let elapsed = started.elapsed();
    let length = duties.len();
    info!(target: "builder_api::get_validators", ?length, ?elapsed, "GET /relay/v1/builder/validators");
    Ok(reply::json(&duties))
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use relay_primitives::beacon::registrations::{
        ValidatorRegistration, ValidatorRegistrationMessage,
    };
    use relay_primitives::blst::public_key::BlsPublicKey;
    use relay_primitives::blst::signature::BlsSignature;
    use relay_primitives::blst::signer::ForkDatas;
    use relay_primitives::events::PayloadAttributesData;

    use relay_primitives::revm_primitives::HashMap;
    use relay_primitives::storage::validator::Validator;
    use relay_primitives::types::Address;
    use relay_primitives::validator::PayloadAttributes;
    use relay_primitives::B256;
    use relay_storage::in_memory::InMemoryStorage;
    use relay_storage::Storage;
    use warp::http::StatusCode;
    use warp::test::request;

    use crate::server::builder_api::get_validators;

    #[tokio::test]
    async fn test_get_validators() {
        let storage = setup_storage();
        storage.set_proposer_duties(vec![validator()]);
        let get_validators = get_validators(storage);
        let resp = request()
            .method("GET")
            .path("/relay/v1/builder/validators")
            .reply(&get_validators)
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.body();
        let duties: Vec<Validator> = serde_json::from_slice(body).unwrap();
        assert_eq!(duties.len(), 1);
        assert_eq!(duties[0].validator_index, 0);
        assert_eq!(duties[0].slot, 0);
        assert_eq!(duties[0].entry.message.fee_recipient, Address::zero());
        let expected_sig = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();
        assert_eq!(duties[0].entry.signature, expected_sig);
    }

    fn validator() -> Validator {
        Validator {
            slot: 0,
            validator_index: 0,
            entry: validator_registration(),
        }
    }

    fn validator_registration() -> ValidatorRegistration {
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();

        ValidatorRegistration {
            signature,
            message: ValidatorRegistrationMessage {
                fee_recipient: Address::zero(),
                gas_limit: 0,
                timestamp: 0,
                pubkey: BlsPublicKey::random(),
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

    fn payload_attr(slot: u64, suggested_fee_recipient: Address) -> PayloadAttributesData {
        PayloadAttributesData {
            proposal_slot: slot,
            parent_block_root: B256::ZERO,
            parent_block_number: 0,
            parent_block_hash: B256::ZERO,
            proposer_index: 0,
            payload_attributes: PayloadAttributes {
                timestamp: 0,
                prev_randao: B256::ZERO,
                suggested_fee_recipient: *suggested_fee_recipient.as_ref(),
                withdrawals: Some(vec![]),
                parent_beacon_block_root: None,
            },
        }
    }
}
