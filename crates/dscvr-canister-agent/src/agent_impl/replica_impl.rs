use std::sync::Arc;
use std::time::Duration;

use candid::Principal;
use ic_agent::Agent;
use ic_agent::Identity;
use instrumented_error::IntoInstrumentedError;
use instrumented_error::Result;
use tokio_retry::strategy::jitter;
use tokio_retry::strategy::ExponentialBackoff;
use tokio_retry::Retry;

use super::AgentImpl;

struct WrappedAgent {
    agent: Agent,
    url: String,
}

impl WrappedAgent {
    async fn fetch_root_key(&self) -> Result<()> {
        let retry_strategy = ExponentialBackoff::from_millis(2000)
            .max_delay(Duration::from_secs(10))
            .map(jitter) // add jitter to delays
            .take(5);

        Ok(Retry::spawn(retry_strategy, move || self.agent.fetch_root_key()).await?)
    }
}

#[async_trait::async_trait]
impl AgentImpl for WrappedAgent {
    async fn query(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>> {
        Ok(self
            .agent
            .query(canister_id, method)
            .with_arg(args)
            .call()
            .await?)
    }

    async fn update(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>> {
        Ok(self
            .agent
            .update(canister_id, method)
            .with_arg(args)
            .call_and_wait()
            .await?)
    }

    fn get_principal(&self) -> Result<Principal> {
        self.agent
            .get_principal()
            .map_err(|e| e.into_instrumented_error())
    }

    async fn clone_with_identity(&self, identity: Arc<dyn Identity>) -> Result<Arc<dyn AgentImpl>> {
        let (route_provider, client) = super::get_route_provider_and_client(&self.url)?;
        let agent = Agent::builder()
            .with_arc_route_provider(route_provider)
            .with_http_client(client)
            .with_max_tcp_error_retries(super::MAX_ERROR_RETRIES)
            .with_arc_identity(identity)
            .with_verify_query_signatures(false)
            .build()?;

        let agent = Arc::new(WrappedAgent {
            agent,
            url: self.url.clone(),
        });

        agent.fetch_root_key().await?;

        Ok(agent)
    }

    async fn read_state_canister_info(
        &self,
        canister_id: &Principal,
        prop: &str,
    ) -> Result<Vec<u8>> {
        Ok(self
            .agent
            .read_state_canister_info(canister_id.to_owned(), prop)
            .await?)
    }
}

pub async fn new<U: Into<String>>(
    identity: Arc<dyn Identity>,
    url: U,
) -> Result<Arc<dyn AgentImpl>> {
    let url_string: String = url.into();
    let (route_provider, client) = super::get_route_provider_and_client(&url_string)?;
    let agent = Agent::builder()
        .with_arc_route_provider(route_provider)
        .with_http_client(client)
        .with_max_tcp_error_retries(super::MAX_ERROR_RETRIES)
        .with_arc_identity(identity)
        .with_verify_query_signatures(false)
        .build()?;

    let agent = Arc::new(WrappedAgent {
        agent,
        url: url_string,
    });

    agent.fetch_root_key().await?;

    Ok(agent)
}
