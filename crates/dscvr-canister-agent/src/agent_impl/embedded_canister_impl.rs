use candid::Principal;
use dscvr_canister_context::{ImmutableContext, MutableContext, UpdateContext};
use dscvr_canister_exports::{CanisterDefinition, CanisterMethod, CanisterUpdateMethod};
use dscvr_interface::edge::Edge;
use ic_agent::Identity;
use instrumented_error::{IntoInstrumentedError, Result};
use std::sync::{Arc, Mutex};
use tracing::debug;

use super::AgentImpl;

/// Implementation that provides a agent-like abstraction a canister that's
/// embedded within the same process via registered exports
struct EmbeddedCanisterImpl<State>
where
    State: std::marker::Send + 'static,
{
    canister: Arc<dscvr_canister_exports::CanisterDefinition<State>>,
    caller: Principal,
    state: Arc<Mutex<State>>,
}

#[async_trait::async_trait]
impl<State> AgentImpl for EmbeddedCanisterImpl<State>
where
    State: std::marker::Send + 'static,
{
    async fn update(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>> {
        let method: &CanisterUpdateMethod<State> =
            self.canister.update_methods.get(method).ok_or_else(|| {
                format!(
                    "Canister {} does not have an update method named {}",
                    canister_id, method
                )
                .into_instrumented_error()
            })?;

        let mut locked_state: std::sync::MutexGuard<State> = self.state.lock().expect("valid");
        let system = Edge::new_with_caller_and_time(self.caller, None);

        method(
            MutableContext::new(&mut locked_state, &system),
            args,
            UpdateContext::Primary,
        )
        .map_err(|e| e.into_instrumented_error())
    }

    async fn query(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>> {
        let method: &CanisterMethod<State> =
            self.canister.query_methods.get(method).ok_or_else(|| {
                format!(
                    "Canister {} does not have an query method named {}",
                    canister_id, method
                )
                .into_instrumented_error()
            })?;

        let locked_state: std::sync::MutexGuard<State> = self.state.lock().expect("valid");
        let system = Edge::new_with_caller_and_time(self.caller, None);

        method(ImmutableContext::new(&locked_state, &system), args)
            .map_err(|e| e.into_instrumented_error())
    }

    async fn read_state_canister_info(
        &self,
        _canister_id: &Principal,
        _prop: &str,
    ) -> Result<Vec<u8>> {
        todo!();
    }

    async fn clone_with_identity(&self, identity: Arc<dyn Identity>) -> Result<Arc<dyn AgentImpl>> {
        Ok(Arc::new(Self {
            canister: self.canister.clone(),
            caller: identity.sender().map_err(|e| e.into_instrumented_error())?,
            state: self.state.clone(),
        }))
    }

    fn get_principal(&self) -> Result<Principal> {
        Ok(self.caller)
    }
}

pub fn new<State>(
    caller: Principal,
    canister: CanisterDefinition<State>,
    init_arguments: Vec<u8>,
    mut state: State,
) -> Arc<dyn AgentImpl>
where
    State: std::marker::Send + 'static,
{
    debug!("Update Method Count: {}", canister.update_methods.len());
    debug!("Query Method Count: {}", canister.query_methods.len());

    let system = Edge::new_with_caller_and_time(caller, None);
    (canister.init_method)(
        MutableContext::new(&mut state, &system),
        &init_arguments,
        UpdateContext::Primary,
    );

    Arc::new(EmbeddedCanisterImpl {
        caller,
        canister: Arc::new(canister),
        state: Arc::new(Mutex::new(state)),
    })
}
