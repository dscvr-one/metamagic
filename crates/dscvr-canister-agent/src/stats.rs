use candid::{Decode, Encode};
use instrumented_error::Result;

use super::CanisterAgent;

impl CanisterAgent {
    /// Return the stats for this canister
    #[tracing::instrument(skip(self))]
    pub async fn canister_stats<Stats>(&self) -> Result<Stats>
    where
        for<'de> Stats: candid::Deserialize<'de>,
        Stats: candid::CandidType,
    {
        let bytes = Encode!()?;
        Ok(Decode!(
            self.query("stats", bytes).await?.as_slice(),
            Stats
        )?)
    }
}
