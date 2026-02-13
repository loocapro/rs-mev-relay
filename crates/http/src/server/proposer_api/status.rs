use crate::server::WarpResult;
use tracing::info;
use warp::Filter;
use warp::Reply;

/// The healthcheck route
pub fn status() -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("eth" / "v1" / "builder" / "status").and_then(handler)
}

async fn handler() -> WarpResult<impl Reply> {
    info!("GET /status");
    Ok::<_, warp::Rejection>(warp::reply::with_status(
        "OK",
        warp::http::status::StatusCode::OK,
    ))
}

#[cfg(test)]
mod tests {

    use warp::http::StatusCode;
    use warp::test::request;

    use crate::server::proposer_api::status;

    #[tokio::test]
    async fn test_status_route() {
        let status = status();
        let resp = request()
            .method("GET")
            .path("/eth/v1/builder/status")
            .reply(&status)
            .await;
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
