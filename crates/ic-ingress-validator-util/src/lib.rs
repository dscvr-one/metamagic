use ic_crypto_utils_threshold_sig_der::parse_threshold_sig_key_from_der;
use instrumented_error::Result;
use std::sync::Arc;

pub use ic_interfaces::crypto::IngressSigVerifier;
pub use ic_validator_ingress_message::IngressMessageVerifier;

pub async fn new_ingress_verifier(url: &str) -> Result<IngressMessageVerifier> {
    use ic_agent::agent::http_transport::ReqwestHttpReplicaV2Transport;
    use ic_agent::identity::AnonymousIdentity;
    use ic_agent::Agent;

    let agent: Agent = Agent::builder()
        .with_transport(ReqwestHttpReplicaV2Transport::create(url)?)
        .with_arc_identity(Arc::new(AnonymousIdentity))
        .build()?;
    agent.fetch_root_key().await?;
    let public_key = parse_threshold_sig_key_from_der(&agent.read_root_key())?;
    Ok(IngressMessageVerifier::builder()
        .with_root_of_trust(public_key)
        .build())
}
