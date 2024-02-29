use instrumented_error::Result;

use super::CanisterAgent;

impl CanisterAgent {
    /// Return the module hash of the canister
    pub async fn canister_module_hash(&self) -> Result<Vec<u8>> {
        self.agent
            .read_state_canister_info(&self.canister_id, "module_hash")
            .await
    }
}
