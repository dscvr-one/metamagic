use candid::Principal;
use ic_agent::Identity;
use instrumented_error::Result;
use std::sync::Arc;

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

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum AgentImplType {
    Default,
    StateMachine,
}
