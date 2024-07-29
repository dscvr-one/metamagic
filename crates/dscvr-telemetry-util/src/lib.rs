pub use axum::{AXUM_HTTP_REQUESTS_DURATION_SECONDS, AXUM_HTTP_REQUESTS_TOTAL};

pub const IC_REPLICA_REQUESTS_TOTAL: &str = "ic-replica-requests-total";
pub const IC_REPLICA_REQUESTS_DURATION_SECONDS: &str = "ic-replica-requests-duration-seconds";

pub mod axum {
    use axum::{extract::MatchedPath, middleware::Next, response::Response, routing::get, Router};
    use http::Request;
    use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder};
    use std::time::Instant;

    pub const AXUM_HTTP_REQUESTS_TOTAL: &str = "axum-http-requests-total";
    pub const AXUM_HTTP_REQUESTS_DURATION_SECONDS: &str = "axum-http-requests-duration-seconds";

    // Takes an existing axum router, installs the prometheus metrics recorder and
    // injects the metrics endpoint into the router after the handler layer is installed so that
    // `/metrics` route itself is not included in the routing layer metrics measured
    pub fn install_metrics_layer<K, S, V>(
        app: Router<S>,
        global_buckets: Option<&[f64]>,
        global_labels: Option<Vec<(K, V)>>,
        matched_metric_buckets: Option<Vec<(&str, &[f64])>>,
    ) -> Result<Router<S>, BuildError>
    where
        K: Into<String>,
        S: Clone + Send + Sync + 'static,
        V: Into<String>,
    {
        let builder = PrometheusBuilder::new();

        let builder = if let Some(buckets) = global_buckets {
            builder.set_buckets(buckets)?
        } else {
            builder
        };

        let builder = if let Some(labels) = global_labels {
            labels
                .into_iter()
                .fold(builder, |b, (k, v)| b.add_global_label(k, v))
        } else {
            builder
        };

        let builder = if let Some(buckets) = matched_metric_buckets {
            buckets.into_iter().try_fold(builder, |b, (k, v)| {
                b.set_buckets_for_metric(Matcher::Full(k.to_owned()), v)
            })?
        } else {
            builder
        };

        let handle = builder.install_recorder()?;
        Ok(app
            .route_layer(axum::middleware::from_fn(track_metrics))
            .route("/metrics", get(|| async move { handle.render() })))
    }

    // Defines a prometheus metrics collection function for defining a tower layer handler
    // as a function. Allows measuring metrics from a router endpoints without needing to expose
    // the metrics endpoint itself on the router or define the endpoint for rendering metrics gathered
    pub async fn track_metrics<B>(req: Request<B>, next: Next<B>) -> Response {
        let start = Instant::now();
        let path = req
            .extensions()
            .get::<MatchedPath>()
            .map(|path| path.as_str().to_owned());
        let method = req.method().clone();

        let response = next.run(req).await;

        if let Some(path) = path {
            let latency = start.elapsed().as_secs_f64();
            let status = response.status().as_u16().to_string();

            let labels = [
                ("method", method.to_string()),
                ("path", path.as_str().to_owned()),
                ("status", status),
            ];

            metrics::counter!(AXUM_HTTP_REQUESTS_TOTAL, &labels).increment(1);
            metrics::histogram!(AXUM_HTTP_REQUESTS_DURATION_SECONDS, &labels).record(latency);
        }

        response
    }
}
