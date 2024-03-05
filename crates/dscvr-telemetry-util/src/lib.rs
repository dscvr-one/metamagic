use axum::{extract::MatchedPath, middleware::Next, response::Response};
use http::Request;
use std::time::Instant;

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

        metrics::counter!("rpc-request-count", &labels).increment(1);
        metrics::histogram!("rpc-request-duration", &labels).record(latency);
    }

    response
}
