use tracing::error;
use warp::{reject::Rejection, reply::Reply};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    builder_api::PostBidsError,
    data_api::GetBidsError,
    proposer_api::{GetHeaderError, PostBlindedBlockErr},
};

/// Errors thrown by the API
#[derive(Error, Debug, Serialize, Deserialize)]
pub enum ApiError {
    /// Errors thrown by the get_header route.
    #[error("{0}")]
    GetHeader(#[from] GetHeaderError),
    /// Errors thrown by the post_blinded route.
    #[error("{0}")]
    PostBlindedBlock(#[from] PostBlindedBlockErr),
    /// Errors thrown by the post bids route.
    #[error("{0}")]
    PostBids(#[from] PostBidsError),
    /// Errors thrown by the get bids route.
    #[error("{0}")]
    GetBids(#[from] GetBidsError),
}

pub(crate) async fn handle_rejection(err: Rejection) -> Result<impl Reply, Rejection> {
    error!("API err: {:?}", err);
    if let Some(err) = err.find::<GetHeaderError>() {
        Ok(warp::reply::with_status(
            warp::reply::json(&ApiError::GetHeader(err.clone()).to_string()),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else if let Some(err) = err.find::<PostBlindedBlockErr>() {
        Ok(warp::reply::with_status(
            warp::reply::json(&ApiError::PostBlindedBlock(err.clone()).to_string()),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else if let Some(err) = err.find::<PostBidsError>() {
        Ok(warp::reply::with_status(
            warp::reply::json(&ApiError::PostBids(err.clone()).to_string()),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else if let Some(err) = err.find::<GetBidsError>() {
        Ok(warp::reply::with_status(
            warp::reply::json(&ApiError::GetBids(err.clone()).to_string()),
            warp::http::StatusCode::BAD_REQUEST,
        ))
    } else {
        Ok(warp::reply::with_status(
            warp::reply::json(&format!("Error: {:?}", err)),
            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
        ))
    }
}
