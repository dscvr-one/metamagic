//! Implements the DSCVR stable storage format that can be used to store and restore canister state.
//! This logic can also be called from off-chain services to deserialize/serialize canister state.
//!
//! The stable storage layout is the following:
//!
//! V2:
//! - Header (serialized as raw binary)
//! - Contents (serialized as bincode or msgpack)
//!
//! V1:
//! - Contents (serialized as msgpack)

pub mod data_format;
#[cfg(not(target_arch = "wasm32"))]
pub mod file_util;
pub mod header;
pub mod interface;
pub mod migration;
pub mod transient;
pub mod v1;
pub mod v2;

pub(crate) use ic_canister_io::movable_io;

/// Stable Storage Error
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)] // self documenting
pub enum Error {
    #[error("msgpack decode {0}")]
    MsgPackDecodeError(#[from] rmp_serde::decode::Error),
    #[error("msgpack encode {0}")]
    MsgPackEncodeError(#[from] rmp_serde::encode::Error),
    #[error("bincode {0}")]
    BincodeError(#[from] bincode::Error),
    #[error("io")]
    Io(#[from] std::io::Error),
    #[error("header")]
    Header(#[from] header::Error),
}

/// Size of a stable storage page
pub const WASM_PAGE_SIZE_IN_BYTES: usize = 64 * 1024; // 64KB
