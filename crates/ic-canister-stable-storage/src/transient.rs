//! State related to stable storage, but that isn't persisted.

use candid::{CandidType, Deserialize};
use serde::Serialize;

/// Transient information related to stable storage
#[derive(Debug, CandidType, Serialize, Deserialize, Default, Clone)]
pub struct Transient {
    /// When set, the next save is skipped
    pub skip_next_save: bool,
    /// Number of instructions used for post-upgrade
    pub post_upgrade_instruction_count: u64,
}
