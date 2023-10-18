//! Logic that wraps around ic_agent's Agent to provide DSCVR specific functionality

use agent_impl::embedded_canister_impl;
use candid::Principal;
use candid::{CandidType, Decode};
use dscvr_canister_config::canister_init_arguments::ControllerType;
use dscvr_canister_config::schema::dscvr::{CanisterNetwork, DSCVRConfig};
use dscvr_canister_exports::CanisterDefinition;
use futures::{stream, StreamExt};
use ic_agent::Identity;
use ic_identity_util::create_identity_from_pem;
use instrumented_error::Result;
use instrumented_error::{IntoInstrumentedError, IntoInstrumentedResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use time::macros::format_description;
use time::OffsetDateTime;
use tracing_error::prelude::*;

mod agent_impl;
mod module_hash;
mod stable_storage_restore_backup;
mod stats;

pub use agent_impl::AgentImpl;

/// The content format stored in stable storage
/// TODO: autogenerate from did
#[derive(Debug, Copy, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum Format {
    /// Unknown
    Unknown = 0,
    /// MsgPack
    MsgPack = 1,
    /// Bincode
    Bincode = 2,
}

/// A wrapper around ic_agent's Agent to provide DSCVR specific functionality
#[derive(Clone)]
pub struct CanisterAgent {
    /// The underlying ic-agent
    agent: Arc<dyn AgentImpl>,
    /// The canister id tied to the agent
    pub canister_id: Principal,
}

impl CanisterAgent {
    /// Return a new context
    #[tracing::instrument]
    pub async fn new(canister_id: &str, pem_file: &Path, url: &str) -> Result<Self> {
        let agent = Self {
            agent: agent_impl::replica_impl::new(create_identity_from_pem(pem_file)?, url).await?,
            canister_id: Principal::from_text(canister_id)?,
        };
        Ok(agent)
    }

    #[tracing::instrument]
    pub fn new_state_machine(
        owner: Principal,
        wasm: Vec<u8>,
        init_arguments: Vec<u8>,
    ) -> Result<Self> {
        let (agent, canister_id) =
            agent_impl::state_machine_impl::new(owner, wasm, init_arguments)?;
        Ok(Self { agent, canister_id })
    }

    #[tracing::instrument(skip(canister, state, init_arguments))]
    pub fn new_embedded_canister<State>(
        caller: Principal,
        canister: CanisterDefinition<State>,
        init_arguments: Vec<u8>,
        state: State,
    ) -> Result<Self>
    where
        State: std::marker::Send + 'static,
    {
        Ok(Self {
            agent: embedded_canister_impl::new(caller, canister, init_arguments, state),
            canister_id: Principal::anonymous(),
        })
    }

    pub async fn new_replica(
        caller: Arc<dyn Identity>,
        replica: &str,
        canister_id: &str,
    ) -> Result<Self> {
        let agent = Self {
            agent: agent_impl::replica_impl::new(caller, replica).await?,
            canister_id: Principal::from_text(canister_id)?,
        };
        Ok(agent)
    }

    pub fn new_from_agent<Agent>(agent: Agent, canister_id: Principal) -> Self
    where
        Agent: AgentImpl + 'static,
    {
        Self {
            agent: Arc::new(agent),
            canister_id,
        }
    }

    pub async fn clone_with_identity(&self, identity: Arc<dyn Identity>) -> Result<Self> {
        Ok(Self {
            agent: self.agent.clone_with_identity(identity).await?,
            canister_id: self.canister_id,
        })
    }

    /// Set the identity for this agent context
    pub async fn set_identity(&mut self, identity: Arc<dyn Identity>) -> Result<()> {
        self.agent = self.agent.clone_with_identity(identity).await?;
        Ok(())
    }

    /// Return a canister URL based off a network configuration
    pub fn get_url(network: &CanisterNetwork) -> Option<String> {
        Some(network.provider.clone())
    }

    /// Return a new context from config and identity.
    #[tracing::instrument(skip_all, fields(canister_name = % canister, network_name = % network_name, instance_name = % instance_name))]
    pub async fn new_from_config_and_identity(
        config: &DSCVRConfig,
        canister: &str,
        instance_name: &str,
        network_name: &str,
        identity: Arc<dyn Identity>,
    ) -> Result<Self> {
        let network = config
            .get_canister_network(canister, network_name)
            .ok_or_else(|| {
                format!(
                    "Could not find canister {} in network {} within config file",
                    canister, network_name
                )
            })
            .into_instrumented_result()?;
        let canister_instance = network
            .find_instance(Some(&instance_name.to_string()), None)
            .ok_or_else(|| {
                format!(
                    "Could not find canister instance {} in network config",
                    instance_name
                )
            })
            .into_instrumented_result()?;
        let canister_id = canister_instance.id.as_ref().ok_or_else(|| {
            format!(
                "Found canister {} but it has no associated id",
                &canister_instance.name
            )
            .into_instrumented_error()
        })?;

        let url = Self::get_url(network).ok_or_else(|| {
            format!("Network {} has no providers", network_name).into_instrumented_error()
        })?;

        let agent = Self {
            agent: agent_impl::replica_impl::new(identity.clone(), &url).await?,
            canister_id: Principal::from_text(canister_id)?,
        };
        Ok(agent)
    }

    /// Return a new context from config
    #[tracing::instrument(skip_all, fields(canister_name = % canister, network_name = % network))]
    pub async fn new_from_config(
        config: &DSCVRConfig,
        canister: &str,
        instance_name: &str,
        network: &str,
        controller: ControllerType,
    ) -> Result<Self> {
        let identity = config
            .get_controller(canister, network, controller)
            .ok_or_else(|| {
                format!(
                    "Controller does not exist for canister {} on network {}",
                    canister, network
                )
            })
            .into_instrumented_result()?
            .identity()?;
        Self::new_from_config_and_identity(config, canister, instance_name, network, identity).await
    }

    pub async fn update<S, A>(&self, method: S, args: A) -> Result<Vec<u8>>
    where
        S: Into<String> + std::marker::Send,
        A: AsRef<[u8]> + std::marker::Send,
    {
        self.agent
            .update(&self.canister_id, &method.into(), args.as_ref())
            .await
    }

    pub async fn query<S, A>(&self, method: S, args: A) -> Result<Vec<u8>>
    where
        S: Into<String> + std::marker::Send,
        A: AsRef<[u8]> + std::marker::Send,
    {
        self.agent
            .query(&self.canister_id, &method.into(), args.as_ref())
            .await
    }

    pub fn get_principal(&self) -> Result<Principal> {
        self.agent.get_principal()
    }
}
