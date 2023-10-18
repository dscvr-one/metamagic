use std::sync::{Arc, Mutex};

use candid::Principal;
use ic_agent::Identity;
use ic_test_state_machine_client::{StateMachine, WasmResult};
use instrumented_error::{IntoInstrumentedError, Result};

use super::AgentImpl;

struct WrappedStateMachine {
    caller: Principal,
    machine: Arc<Mutex<StateMachine>>,
    canister_id: Principal,
}

#[async_trait::async_trait]
impl AgentImpl for WrappedStateMachine {
    async fn query(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>> {
        let state_machine = self.machine.lock().unwrap();
        match state_machine
            .query_call(
                canister_id.to_owned(),
                self.caller.to_owned(),
                method,
                args.to_owned(),
            )
            .map_err(|e| e.to_string().into_instrumented_error())?
        {
            WasmResult::Reply(reply) => Ok(reply),
            WasmResult::Reject(reject) => Err(reject.into_instrumented_error()),
        }
    }

    async fn update(&self, canister_id: &Principal, method: &str, args: &[u8]) -> Result<Vec<u8>> {
        let state_machine = self.machine.lock().unwrap();
        match state_machine
            .update_call(
                canister_id.to_owned(),
                self.caller.to_owned(),
                method,
                args.to_owned(),
            )
            .map_err(|e| e.to_string().into_instrumented_error())?
        {
            WasmResult::Reply(reply) => Ok(reply),
            WasmResult::Reject(reject) => Err(reject.into_instrumented_error()),
        }
    }

    fn get_principal(&self) -> Result<Principal> {
        Ok(self.caller.to_owned())
    }

    async fn clone_with_identity(&self, identity: Arc<dyn Identity>) -> Result<Arc<dyn AgentImpl>> {
        Ok(Arc::new(WrappedStateMachine {
            caller: identity.sender().map_err(|e| e.into_instrumented_error())?,
            machine: self.machine.clone(),
            canister_id: self.canister_id,
        }))
    }

    async fn read_state_canister_info(
        &self,
        _canister_id: &Principal,
        _prop: &str,
    ) -> Result<Vec<u8>> {
        unimplemented!()
    }
}

pub fn new(
    caller: Principal,
    wasm: Vec<u8>,
    init_arguments: Vec<u8>,
) -> Result<(Arc<dyn AgentImpl>, Principal)> {
    // TODO: for multi-canister WrappedStateMachine needs to be a singleton
    let machine = Arc::new(Mutex::new(StateMachine::new(
        &std::env::var("STATE_MACHINE_BINARY_PATH").expect("valid state machine binary path"),
        false,
    )));

    let canister_id = {
        let machine = machine.lock().unwrap();
        let canister_id = machine.create_canister(Some(caller));
        machine.install_canister(canister_id, wasm, init_arguments, Some(caller));
        canister_id
    };

    Ok((
        Arc::new(WrappedStateMachine {
            caller,
            machine,
            canister_id,
        }),
        canister_id,
    ))
}
