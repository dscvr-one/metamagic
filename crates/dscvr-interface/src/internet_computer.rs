use crate::{Interface, Principal};
use ic_cdk::call::RejectCode;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread::spawn;

pub const SYSTEM: &dyn Interface = &InternetComputer;

#[derive(Default)]
pub struct InternetComputer;

impl Interface for InternetComputer {
    fn time(&self) -> u64 {
        ic_cdk::api::time()
    }

    fn caller(&self) -> Principal {
        ic_cdk::api::msg_caller()
    }

    fn canister_balance(&self) -> u64 {
        ic_cdk::api::canister_cycle_balance() as u64
    }

    fn call_canister(
        &self,
        canister_id: Principal,
        method: String,
        args: Vec<u8>,
        payment: u64,
    ) -> Result<Vec<u8>, (RejectCode, String)> {
        // Ideally ic_cdk::spawn would allow returning a result, but it doesn't. so we go through
        // some gymanistics to make it work.
        let result: Rc<RefCell<Result<Vec<u8>, (RejectCode, String)>>> = Rc::new(RefCell::new(
            Err((RejectCode::CanisterReject, "spawn failed".to_owned())),
        ));
        {
            let caller_result = result.clone();
            ic_cdk::futures::spawn(async move {
                let result = ic_cdk::call::Call::unbounded_wait(canister_id, &method)
                    .with_arg(&args)
                    .with_cycles(payment as u128)
                    .await
                    .map_err(|e| (RejectCode::CanisterReject, e.to_string()));
                let _ = caller_result.replace(result.map(|f| f.into_bytes()));
            });
        }
        let mut mut_borrow = result.borrow_mut();
        match &mut *mut_borrow {
            Ok(result) => Ok(std::mem::take(result)),
            Err((code, s)) => Err((code.clone(), std::mem::take(s))),
        }
    }

    fn id(&self) -> Principal {
        ic_cdk::api::canister_self()
    }
    fn get_memory_usage(&self) -> u64 {
        (core::arch::wasm32::memory_size(0) * 65536) as u64
    }

    fn performance_counter(&self, counter_type: u32) -> u64 {
        ic_cdk::api::performance_counter(counter_type)
    }

    fn instruction_counter(&self) -> u64 {
        ic_cdk::api::instruction_counter()
    }

    fn stable64_size(&self) -> u64 {
        ic_cdk::stable::stable_size()
    }
}
