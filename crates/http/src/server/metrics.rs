use metrics_exporter_prometheus::PrometheusBuilder;
use std::{net::SocketAddr, sync::Arc};
use tracing::info;
use warp::Filter;

/// The Metrics server
#[derive(Debug)]
pub struct MetricsServer;

impl MetricsServer {
    /// Spawns a Http metrics server on a given address.
    /// server. This works on top of a Prometheus exporter. It allows us to serve
    /// metrics requests in the form of a Prometheus scraper endpoint.
    ///
    /// NOTE: This function must be called before application metrics are registered.
    /// All metrics registered before the initialization of the exporter will not
    /// be tracked. Run this function as earliest as possible in the application lifecycle.
    pub async fn spawn(addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>> {
        info!(target: "server::metrics",?addr, "Metrics server started");
        let builder = PrometheusBuilder::new();
        let handle = builder.install_recorder()?;
        let handle = Arc::new(handle);

        let route = warp::path!("metrics")
            .and(warp::get())
            .and(filters::with_handle(handle))
            .and_then(handlers::get_metrics);

        warp::serve(route).run(addr).await;
        Ok(())
    }
}

mod handlers {
    use metrics_exporter_prometheus::PrometheusHandle;
    use std::{convert::Infallible, sync::Arc};

    /// Returns a `text/plain` response containing the status of
    /// all the collected metrics. The Prometheus `handle` is crucial
    /// to get the current snapshot of all tracked metrics.
    pub(crate) async fn get_metrics(
        handle: Arc<PrometheusHandle>,
    ) -> Result<impl warp::Reply, Infallible> {
        let metrics: String = handle.render();
        Ok(warp::reply::with_header(
            metrics,
            warp::http::header::CONTENT_TYPE,
            "text/plain; charset=UTF-8",
        ))
    }
}

mod filters {
    use metrics_exporter_prometheus::PrometheusHandle;
    use std::sync::Arc;
    use warp::Filter;

    pub(crate) fn with_handle(
        handle: Arc<PrometheusHandle>,
    ) -> impl Filter<Extract = (Arc<PrometheusHandle>,), Error = std::convert::Infallible> + Clone
    {
        warp::any().map(move || handle.clone())
    }
}
