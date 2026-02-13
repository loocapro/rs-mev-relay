use crate::BeaconApi;
use relay_primitives::beacon::fork::ForkName;
use relay_primitives::beacon::known_validator::KnownValidatorResponse;
use relay_primitives::beacon::{
    beacon_block::SignedBeaconBlockContent, fork::GetForkResponse,
    proposer_duties::ProposerDutiesResponse, sync_status::BeaconSyncStatusResponse,
};
use relay_primitives::blst::public_key::BlsPublicKey;
use reqwest::header::CONTENT_TYPE;
use reqwest::{header::HeaderMap, Client};
use serde::Deserialize;
use serde::{de::DeserializeOwned, Serialize};
use ssz::Encode;
use tracing::debug;
use url::Url;
/// The type of the body during the submission of a beacon block.
pub enum SubmissionType {
    /// The body is a JSON object.
    Json,
    /// The body is a SSZ encoded object.
    Ssz,
}

impl BeaconApi for BeaconNodeApi {
    type Config = Url;

    fn new(url: Self::Config) -> Self {
        Self {
            client: Client::new(),
            url,
        }
    }

    async fn sync_status(&self) -> eyre::Result<BeaconSyncStatusResponse> {
        self.get_api_response(SYNC_STATUS_PATH).await
    }

    async fn proposer_duties(&self, epoch: u64) -> eyre::Result<ProposerDutiesResponse> {
        let path = format!("{}{}", PROPOSER_DUTIES_PATH, epoch);
        self.get_api_response(&path).await
    }

    async fn publish_block(
        &self,
        block: SignedBeaconBlockContent,
        submission_type: SubmissionType,
    ) -> eyre::Result<()> {
        self.post_api_response(PUBLISH_BLOCK, &block, submission_type)
            .await?;
        Ok(())
    }
    async fn get_fork_data(&self) -> eyre::Result<GetForkResponse> {
        let fork_data = self.get_api_response(GET_FORK_DATA).await?;
        Ok(fork_data)
    }
    async fn get_known_validator(
        &self,
        pubkey: BlsPublicKey,
    ) -> eyre::Result<KnownValidatorResponse> {
        let path = format!("{}{}", GET_VALIDATOR, pubkey);
        let validator = self.get_api_response(&path).await?;
        Ok(validator)
    }
}

// Beacon Node API Endpoints

/// Returns the sync status of a beacon node.
const SYNC_STATUS_PATH: &str = "eth/v1/node/syncing";

/// Returns the proposer duties of the beacon node for a given epoch.
const PROPOSER_DUTIES_PATH: &str = "eth/v1/validator/duties/proposer/";

/// Publish a block to the beacon node.
///
/// https://ethereum.github.io/beacon-APIs/#/Beacon/publishBlock
const PUBLISH_BLOCK: &str = "eth/v1/beacon/blocks";

/// Returns the fork data of the beacon node.
const GET_FORK_DATA: &str = "eth/v2/debug/beacon/states/head";

/// Returns a consensus validator by its validator_id, which can be either the validator_index or the validator pubkey.
///
/// https://ethereum.github.io/beacon-APIs/#/Beacon/getStateValidator
const GET_VALIDATOR: &str = "eth/v1/beacon/states/head/validators/";

/// Beacon api entry point.
#[derive(Debug, Clone)]
pub struct BeaconNodeApi {
    client: Client,
    url: Url,
}

impl BeaconNodeApi {
    /// Creates a new Api instance with default configuration.
    pub fn new(url: &Url) -> Self {
        Self {
            client: Client::new(),
            url: url.to_owned(),
        }
    }

    // Centralized method to perform GET requests
    async fn get_api_response<T: DeserializeOwned>(&self, path: &str) -> eyre::Result<T> {
        let url = format!("{}{}", self.url, path);
        debug!(target:"beacon-client", ?url, "calling beacon api");
        let resp = self.client.get(&url).send().await?;

        // Check if the status code is a success
        if resp.status().is_success() {
            // Deserialize the successful response
            resp.json::<T>().await.map_err(From::from)
        } else {
            // Deserialize the error response
            let error_resp = resp.json::<ApiErrorResponse>().await?;
            Err(eyre::eyre!(
                "API error: code {}, message: {}",
                error_resp.code,
                error_resp.message
            ))
        }
    }

    // Centralized method to perform POST requests
    async fn post_api_response<B: Serialize + Encode>(
        &self,
        path: &str,
        body: &B,
        submission_type: SubmissionType,
    ) -> eyre::Result<()> {
        let url = format!("{}{}", self.url, path);

        let mut headers = HeaderMap::new();
        headers.insert(
            "Eth-Consensus-Version",
            ForkName::Deneb.to_string().parse().unwrap(),
        );

        let client = self.client.post(&url);

        match submission_type {
            SubmissionType::Json => client.headers(headers).json(body).send().await?,
            SubmissionType::Ssz => {
                headers.insert(CONTENT_TYPE, "application/octet-stream".parse().unwrap());
                client
                    .headers(headers)
                    .body(body.as_ssz_bytes())
                    .send()
                    .await?
            }
        };
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiErrorResponse {
    code: u16,
    message: String,
    stacktraces: Vec<String>,
}
