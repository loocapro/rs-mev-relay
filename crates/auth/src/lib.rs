//! # Auth crate
//! Used to perform authentication against an external auth service (validates API keys for proposer endpoints).
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::future::Future;
use thiserror::Error;
use tracing::{error, info};
use url::Url;
use uuid::Uuid;

/// Auth error type.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Request error.
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
    /// Invalid secret.
    #[error("Invalid secret")]
    InvalidSecret,
}

#[derive(Debug, Clone)]
/// MbsAuth struct.
pub struct MbsAuth {
    /// Auth service url.
    pub url: Url,
    /// Reqwest client.
    pub client: Client,
}

impl MbsAuth {
    /// Create a new MbsAuth instance.
    pub fn new(url: Url) -> Result<Self, AuthError> {
        let client = reqwest::Client::builder().build()?;
        Ok(Self { url, client })
    }
}

impl Auth for MbsAuth {
    async fn validate(&self, secret: String) -> Result<MbsUser, AuthError> {
        let full_url = format!("{}get-key", self.url);
        let body: SecretBody = SecretBody { secret };
        let request = self
            .client
            .request(reqwest::Method::POST, full_url)
            .json(&body);
        let resp = request.send().await?;
        if resp.status() == 200 {
            let data = resp.json::<MbsUser>().await?;
            info!(?data, "Authenticated");
            Ok(data)
        } else {
            error!(
                "Auth err: code: {} and msg: {}",
                resp.status(),
                resp.text().await?
            );
            Err(AuthError::InvalidSecret)
        }
    }
}

/// This trait defines the behavior of an HTTP authentication service.
pub trait Auth {
    /// Authenticate a secret and return a user.
    fn validate(&self, secret: String) -> impl Future<Output = Result<MbsUser, AuthError>> + Send;
}
/// MbsUser struct.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct MbsUser {
    /// Team id.
    #[serde(rename = "teamId")]
    pub team_id: Uuid,
    /// User id.
    pub id: Uuid,
}

/// SecretBody struct.
#[derive(Serialize, Debug)]
pub struct SecretBody {
    /// The API secret from your auth provider (sent to auth.url for validation).
    pub secret: String,
}
