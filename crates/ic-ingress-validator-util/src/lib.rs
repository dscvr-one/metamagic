use ic_agent::agent::http_transport::ReqwestTransport;
use ic_agent::identity::AnonymousIdentity;
use ic_agent::Agent;
use ic_crypto_utils_threshold_sig_der::parse_threshold_sig_key_from_der;
use ic_types::messages::UserQuery;
use ic_validator_ingress_message::{HttpRequestVerifier, IngressMessageVerifier};
use instrumented_error::Result;
use std::sync::Arc;

pub type IcHttpRequestVerifier = Arc<dyn HttpRequestVerifier<UserQuery> + Send + Sync>;

pub async fn try_new_ingress_verifier(url: &str) -> Result<IcHttpRequestVerifier> {
    let agent: Agent = Agent::builder()
        .with_transport(ReqwestTransport::create(url)?)
        .with_arc_identity(Arc::new(AnonymousIdentity))
        .build()?;
    agent.fetch_root_key().await?;
    let public_key = parse_threshold_sig_key_from_der(&agent.read_root_key())?;
    Ok(Arc::new(
        IngressMessageVerifier::builder()
            .with_root_of_trust(public_key)
            .build(),
    ))
}
