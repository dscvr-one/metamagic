/// Setup DSCVR service tracing for GCP
pub fn setup_gcp_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::EnvFilter;

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let stackdriver = tracing_stackdriver::layer(); // writes to std::io::Stdout

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(stackdriver)
        .with(ErrorLayer::default())
        .init();
}

/// Setup the common tracing configuration
pub fn setup_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::{prelude::*, EnvFilter, Registry};
    use tracing_tree::HierarchicalLayer;

    Registry::default()
        .with(
            HierarchicalLayer::default()
                .with_verbose_entry(false)
                .with_verbose_exit(false)
                .with_targets(true)
                .with_bracketed_fields(true)
                .with_filter(EnvFilter::from_default_env()),
        )
        .with(ErrorLayer::default())
        .init();
}

/// Recrusively log the top-level error and all its sources
pub fn err_to_string(e: impl std::error::Error) -> String {
    let mut s = format!("{:?}", e);
    let mut e = e.source();
    while let Some(ee) = e {
        s.push_str(&format!("\n{:?}", ee));
        e = ee.source();
    }
    s
}
