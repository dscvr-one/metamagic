//! Initialization arguments for canisters

use std::collections::HashMap;

use candid::{CandidType, Deserialize, Principal};
use serde::Serialize;

/// The initialization arguments for a canister.
/// These are copy/pasted from the canister model
// TODO: generate from did
#[derive(Clone, Debug, Eq, PartialEq, CandidType, Deserialize, Serialize, Default)]
#[allow(missing_docs)] // TODO: add more detailed docs after finalizing generation
pub struct InitArguments {
    pub keys: HashMap<ControllerType, Principal>,
}

/// Controller types for a canister
// TODO: generate from did
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Hash, Clone, Copy, CandidType)]
#[serde(rename_all = "lowercase")]
#[allow(missing_docs)] // TODO: add more detailed docs after finalizing generation
pub enum ControllerType {
    Backup,
    Restore,
    HCaptchaVerify,
    PhoneVerify,
    Gating,
    Mod,
    Owner,
    EventProducer,
    EventRouter,
    TxLogConsumer,
    TxLogProducer,
}
