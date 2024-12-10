use candid::Principal;
use ic_agent::agent::route_provider::RoundRobinRouteProvider;
use ic_agent::Identity;
use instrumented_error::Result;
use reqwest::Client;
use std::sync::Arc;

pub const MAX_ERROR_RETIRES: usize = 3;

pub mod embedded_canister_impl;
pub mod replica_impl;
pub mod state_machine_impl;

/// Abstracts agent-rs and ic-state-machine-client to allow reusing logic to seamlessly interact
/// for both integration tests, test replica, and the mainnet.
#[async_trait::async_trait]
pub trait AgentImpl: Sync + Send {
    async fn update(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>>;

    async fn query(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>>;

    async fn read_state_canister_info(
        &self,
        canister_id: &Principal,
        prop: &str,
    ) -> Result<Vec<u8>>;

    async fn clone_with_identity(&self, identity: Arc<dyn Identity>) -> Result<Arc<dyn AgentImpl>>;

    fn get_principal(&self) -> Result<Principal>;
}

pub fn get_route_provider_and_client(url: &str) -> Result<(Arc<RoundRobinRouteProvider>, Client)> {
    let route_provider = Arc::new(RoundRobinRouteProvider::new(vec![url])?);
    let client = Client::builder().use_rustls_tls().build()?;
    Ok((route_provider, client))
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum AgentImplType {
    Default,
    StateMachine,
}
