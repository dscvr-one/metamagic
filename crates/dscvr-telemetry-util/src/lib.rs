pub mod axum {
    use axum::{extract::MatchedPath, middleware::Next, response::Response, routing::get, Router};
    use http::Request;
    use metrics_exporter_prometheus::{BuildError, PrometheusBuilder};
    use std::time::Instant;

    // Takes an existing axum router, installs the prometheus metrics recorder and
    // injects the metrics endpoint into the router after the handler layer is installed so that
    // `/metrics` route itself is not included in the routing layer metrics measured
    pub fn install_metrics_layer<K, V>(
        app: Router,
        bucket_vals: Option<&[f64]>,
        global_labels: Option<Vec<(K, V)>>,
    ) -> Result<Router, BuildError>
    where
        K: Into<String>,
        V: Into<String>,
    {
        let builder = PrometheusBuilder::new();

        let builder = if let Some(buckets) = bucket_vals {
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

            metrics::counter!("axum-http-requests-total", &labels).increment(1);
            metrics::histogram!("axum-http-requests-duration-seconds", &labels).record(latency);
        }

        response
    }
}
