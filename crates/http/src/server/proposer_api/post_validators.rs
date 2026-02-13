use crate::server::with_auth;
use crate::server::with_beacon_handle;
use crate::server::with_json_body;
use crate::server::with_storage;
use crate::server::WarpResult;
use beacon::service::handle::BeaconHandle;
use relay_auth::Auth;
use relay_primitives::beacon::registrations::ValidatorRegistration;
use relay_primitives::blst::SignedRoot;
use relay_primitives::hex;
use relay_primitives::revm_primitives::HashMap;
use relay_storage::Storage;
use serde::Deserialize;
use tokio::time::Instant;
use tracing::error;
use tracing::info;
use warp::reject::Rejection;
use warp::Filter;
use warp::Reply;

pub(crate) async fn validate_api_key(
    query_param: ApiKeyQuery,
    auth: impl Auth,
) -> Result<String, Rejection> {
    let api_key = query_param.api_key;
    auth.validate(api_key.clone()).await.map_err(|e| {
        error!(?e, "Failed to validate api key");
        warp::reject::reject()
    })?;
    Ok(api_key)
}

/// Struct to represent the query parameter
#[derive(Deserialize, Clone)]
pub(crate) struct ApiKeyQuery {
    api_key: String,
}

/// Route for POST `/eth/v1/builder/validators`
///
/// See also <https://ethereum.github.io/builder-specs/#/Builder/registerValidator>
pub fn post_validators(
    auth: impl Auth + Clone + Send + Sync + 'static,
    storage: impl Storage + Clone + Send + Sync,
    beacon_handle: BeaconHandle,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let payload_limit = 1024 * 1024 * 80; // 80mb
    warp::path!("eth" / "v1" / "builder" / "validators")
        .and(warp::post())
        .and(warp::query::<ApiKeyQuery>())
        .and(with_auth(auth))
        .and_then(validate_api_key)
        .and(with_storage(storage))
        .and(with_beacon_handle(beacon_handle))
        .and(with_json_body::<Vec<ValidatorRegistration>>(payload_limit))
        .and_then(handler)
}

async fn handler<S: Storage + Clone>(
    _api_key: String,
    storage: S,
    beacon_handle: BeaconHandle,
    registrations: Vec<ValidatorRegistration>,
) -> WarpResult<impl Reply> {
    let length = registrations.len();
    let started = Instant::now();
    let domain = storage.fork_data().compute_builder_domain();
    let mut registered = 0;
    let mut to_register = HashMap::new();
    for reg in registrations {
        let pubkey = reg.message.pubkey;

        // Skip inactive or unknown validators
        match beacon_handle.get_known_validator(pubkey).await {
            Ok(validator) => {
                if !validator.is_active() {
                    error!(?pubkey, "Validator is not active skipping.");
                    continue;
                }
            }
            Err(err) => {
                error!(?err, ?pubkey, "Not a known validator skipping.");
                continue;
            }
        }

        let invalid_sig = !reg
            .signature
            .verify(&pubkey, reg.message.signing_root(domain.into()));
        if invalid_sig {
            let hex = hex::encode(domain);
            error!(?reg, ?hex, "Invalid sig");
            continue;
        }
        to_register.insert(pubkey, reg);
        registered += 1;
    }
    storage.set_validator_registrations(to_register);

    let elapsed = started.elapsed();
    info!(
        target: "proposer_api::post_validators",
        ?length,
        ?elapsed,
        ?registered,
        "POST /eth/v1/builder/validators"
    );

    Ok::<_, warp::Rejection>(warp::reply::with_status(
        "OK",
        warp::http::status::StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {
    use crate::server::proposer_api::post_validators;
    use crate::server::tests::TestApi;
    use crate::server::tests::TestAuth;
    use beacon::service::handle::BeaconHandle;
    use beacon::service::BeaconService;
    use beacon::BeaconApi;
    use relay_primitives::beacon::registrations::ValidatorRegistration;
    use relay_primitives::beacon::registrations::ValidatorRegistrationMessage;
    use relay_primitives::blst::public_key::BlsPublicKey;
    use relay_primitives::blst::signature::BlsSignature;
    use relay_primitives::blst::signer::BlsSigner;
    use relay_primitives::blst::signer::ForkDatas;
    use relay_primitives::events::PayloadAttributesData;
    use relay_primitives::revm_primitives::HashMap;
    use relay_primitives::types::Address;
    use relay_primitives::validator::PayloadAttributes;
    use relay_primitives::B256;
    use relay_storage::in_memory::InMemoryStorage;
    use relay_storage::Storage;

    use std::str::FromStr;
    use std::sync::Arc;
    use warp::http::StatusCode;
    use warp::test::request;

    #[tokio::test]
    async fn test_post_validators() {
        let storage = setup_storage();
        let api = Arc::new(TestApi::new(()));
        let (beacon_service, beacon_handle) = BeaconService::new(api);
        tokio::task::spawn(beacon_service);

        invalid_by_sig(storage.clone(), beacon_handle.clone()).await;

        let registrations = generate_signed_registrations(50);
        valid(storage.clone(), beacon_handle.clone(), registrations).await;
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

    async fn invalid_by_sig<S: Storage + Clone + Send + Sync + 'static>(
        storage: S,
        beacon_handle: BeaconHandle,
    ) {
        let signature = BlsSignature::from_str(relay_primitives::test_utils::TEST_BLS_SIGNATURE).unwrap();
        let signer = BlsPublicKey::random();
        let registations = validator_registration(signer, Some(signature));

        let post_validators = post_validators(TestAuth, storage.clone(), beacon_handle);
        let resp = request()
            .method("POST")
            .path("/eth/v1/builder/validators?api_key=123")
            .json(&vec![registations])
            .reply(&post_validators)
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    async fn valid<S: Storage + Clone + Send + Sync + 'static>(
        storage: S,
        beacon_handle: BeaconHandle,
        registrations: Vec<ValidatorRegistration>,
    ) {
        let post_validators = post_validators(TestAuth, storage.clone(), beacon_handle);
        let resp = request()
            .method("POST")
            .path("/eth/v1/builder/validators?api_key=123")
            .json(&registrations)
            .reply(&post_validators)
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }

    fn generate_signed_registrations(num_regs: usize) -> Vec<ValidatorRegistration> {
        let registrations: Vec<ValidatorRegistration> = (0..num_regs)
            .map(|_| {
                let signer = BlsSigner::default();
                let pubkey = signer.public_key();
                let mut registration: ValidatorRegistration = validator_registration(pubkey, None);
                let signature = signer.sign(&mut registration.message);
                registration.signature = signature;
                registration
            })
            .collect();

        registrations
    }

    fn validator_registration(
        pubkey: BlsPublicKey,
        signature: Option<BlsSignature>,
    ) -> ValidatorRegistration {
        ValidatorRegistration {
            signature: signature.unwrap_or_default(),
            message: ValidatorRegistrationMessage {
                fee_recipient: Address::random(),
                gas_limit: 0,
                timestamp: 0,
                pubkey,
            },
        }
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
