use candid::Principal;
use ic_cdk::api::call::RejectionCode;

#[cfg(not(target_arch = "wasm32"))]
pub mod edge;
#[cfg(target_arch = "wasm32")]
pub mod internet_computer;
#[cfg(not(target_arch = "wasm32"))]
pub mod unit_test;

pub trait Interface: Send + Sync {
    fn time(&self) -> u64;
    fn caller(&self) -> Principal;
    fn canister_balance(&self) -> u64;
    fn call_canister(
        &self,
        canister_id: Principal,
        method: String,
        args: Vec<u8>,
        payment: u64,
    ) -> Result<Vec<u8>, (RejectionCode, String)>;
    fn id(&self) -> Principal;
    fn get_memory_usage(&self) -> u64;
    fn performance_counter(&self, counter_type: u32) -> u64;
    fn instruction_counter(&self) -> u64;
    fn stable64_size(&self) -> u64;
}
