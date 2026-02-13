use beacon::service::handle::BeaconHandle;
use relay_auth::Auth;
use relay_primitives::blst::signer::BlsSigner;
use relay_storage::Storage;
use serde::de::DeserializeOwned;
use std::net::SocketAddr;

use warp::{reject::Rejection, Filter};

use self::errors::handle_rejection;

/// The Builder api routes
pub mod builder_api;
/// The Proposer api routes
pub mod proposer_api;

/// The Data api routes
pub mod data_api;

/// The Metrics server
pub mod metrics;

/// The HTTP server errors
pub mod errors;

type WarpResult<T> = std::result::Result<T, Rejection>;

/// The relay HTTP server
#[derive(Debug)]
pub struct RelayHttpServer;

impl RelayHttpServer {
    /// Spawns the HTTP server
    pub async fn start<S: Storage + Clone + Send + Sync + 'static>(
        addr: SocketAddr,
        storage: S,
        beacon_handle: BeaconHandle,
        signer: BlsSigner,
        auth: impl Auth + Clone + Send + Sync + 'static,
    ) {
        let proposer_api = proposer_api::status()
            .or(proposer_api::post_blinded_blocks(
                storage.clone(),
                beacon_handle.clone(),
            ))
            .or(proposer_api::get_header(storage.clone(), signer))
            .or(proposer_api::post_validators(
                auth.clone(),
                storage.clone(),
                beacon_handle.clone(),
            ));

        let builder_api = builder_api::get_validators(storage.clone())
            .or(builder_api::post_bids(storage.clone()));

        let data_api = data_api::get_bids(storage);

        let cors = warp::cors().allow_any_origin();

        warp::serve(
            proposer_api
                .or(builder_api)
                .or(data_api)
                .recover(handle_rejection)
                .with(cors),
        )
        .run(addr)
        .await;
    }
}

fn with_json_body<T: DeserializeOwned + Send>(
    size_limit: u64,
) -> impl Filter<Extract = (T,), Error = Rejection> + Clone {
    // accept json body and limit payload size to 16kb
    warp::body::content_length_limit(size_limit).and(warp::body::json())
}
fn with_storage<S: Storage + Clone + Send + Sync>(
    storage: S,
) -> impl Filter<Extract = (S,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || storage.clone())
}

fn with_auth(
    auth: impl Auth + Clone + std::marker::Send,
) -> impl Filter<Extract = (impl Auth,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || auth.clone())
}

fn with_signer(
    signer: BlsSigner,
) -> impl Filter<Extract = (BlsSigner,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || signer.clone())
}

fn with_beacon_handle(
    beacon_handle: BeaconHandle,
) -> impl Filter<Extract = (BeaconHandle,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || beacon_handle.clone())
}

#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use crate::server::builder_api::{self};
    use crate::server::errors::handle_rejection;
    use crate::server::{data_api, proposer_api};
    use beacon::api::SubmissionType;
    use beacon::service::BeaconService;
    use beacon::BeaconApi;
    use relay_auth::{Auth, AuthError, MbsUser};
    use relay_primitives::beacon::beacon_block::SignedBeaconBlockContent;
    use relay_primitives::beacon::fork::GetForkResponse;
    use relay_primitives::beacon::known_validator::KnownValidatorResponse;
    use relay_primitives::beacon::proposer_duties::ProposerDutiesResponse;
    use relay_primitives::beacon::sync_status::BeaconSyncStatusResponse;

    use relay_primitives::blst::public_key::BlsPublicKey;
    use relay_primitives::blst::signer::ForkDatas;
    use relay_primitives::events::PayloadAttributesData;
    use relay_primitives::revm_primitives::HashMap;
    use relay_primitives::types::Address;
    use relay_primitives::validator::PayloadAttributes;
    use relay_primitives::B256;
    use relay_storage::in_memory::InMemoryStorage;
    use uuid::Uuid;
    use warp::http::StatusCode;
    use warp::test::request;
    use warp::Filter;

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

    fn known_validators_response() -> KnownValidatorResponse {
        const PH: &str = "{{TEST_BLS_PUBKEY}}";
        let known_validator_json = r#"{
    "execution_optimistic": false,
    "finalized": true,
    "data": 
        {
            "index": "1",
            "balance": "1",
            "status": "active",
            "validator": {
                "pubkey": "{{TEST_BLS_PUBKEY}}",
                "withdrawal_credentials": "WithdrawalCred1",
                "effective_balance": "1",
                "slashed": false,
                "activation_eligibility_epoch": "0",
                "activation_epoch": "0",
                "exit_epoch": "12341431",
                "withdrawable_epoch": "1213413"
            }
        }
        }"#;
        let json = known_validator_json.replace(PH, relay_primitives::test_utils::test_bls_pubkey_str());
        serde_json::from_str(&json).unwrap()
    }
    #[derive(Clone)]
    pub(crate) struct TestApi;

    impl BeaconApi for TestApi {
        type Config = ();

        fn new(_config: Self::Config) -> Self {
            TestApi
        }
        async fn get_known_validator(
            &self,
            _pubkey: BlsPublicKey,
        ) -> eyre::Result<KnownValidatorResponse> {
            Ok(known_validators_response())
        }
        async fn sync_status(&self) -> eyre::Result<BeaconSyncStatusResponse> {
            unimplemented!()
        }

        async fn proposer_duties(&self, _epoch: u64) -> eyre::Result<ProposerDutiesResponse> {
            unimplemented!()
        }
        async fn publish_block(
            &self,
            _block: SignedBeaconBlockContent,
            _submission_type: SubmissionType,
        ) -> eyre::Result<()> {
            unimplemented!()
        }

        async fn get_fork_data(&self) -> eyre::Result<GetForkResponse> {
            unimplemented!()
        }
    }

    #[derive(Clone)]
    pub(crate) struct TestAuth;

    impl Auth for TestAuth {
        async fn validate(&self, _secret: String) -> Result<MbsUser, AuthError> {
            Ok(MbsUser {
                team_id: Uuid::new_v4(),
                id: Uuid::new_v4(),
            })
        }
    }
    #[tokio::test]
    async fn test_server() {
        let storage = InMemoryStorage::new(
            0.into(),
            payload_attr(0, Address::random()).into(),
            vec![],
            ForkDatas::default(),
            HashMap::new(),
            vec![],
        );

        let api = Arc::new(TestApi::new(()));
        let (_beacon_service, beacon_handle) = BeaconService::new(api);
        let proposer_api = proposer_api::status()
            .or(proposer_api::post_blinded_blocks(
                storage.clone(),
                beacon_handle.clone(),
            ))
            .or(proposer_api::post_validators(
                TestAuth,
                storage.clone(),
                beacon_handle,
            ));

        let builder = builder_api::get_validators(storage.clone());
        let data = data_api::get_bids(storage);
        let routes = proposer_api.or(builder).or(data).recover(handle_rejection);
        let resp = request()
            .method("GET")
            .path("/relay/v1/builder/validators")
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);

        let resp = request()
            .method("GET")
            .path("/eth/v1/builder/status")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), StatusCode::OK);

        let resp = request()
            .method("POST")
            .path("/relay/v1/data/bidtraces/builder_blocks_received")
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
    }
}
