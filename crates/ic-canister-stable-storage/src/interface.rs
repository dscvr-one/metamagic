//! Common stable storage logic for use in canisters

use ic_cdk::api::stable::StableReader;
use ic_cdk::api::stable::StableWriter;
use serde_bytes::ByteBuf;
use std::cell::RefCell;
use std::io::Read;
use tracing::info;

use crate::Error;
use crate::{header::Header, transient::Transient, WASM_PAGE_SIZE_IN_BYTES};

thread_local! {
    static HEADER: RefCell<Header> = RefCell::default();
    static TRANSIENT: RefCell<Transient> = RefCell::default();
}

/// Return the stable storage header and transient structures
#[inline]
pub fn stable_storage_info() -> (Header, Transient) {
    (
        HEADER.with(|h| h.borrow().clone()),
        TRANSIENT.with(|t| t.borrow().clone()),
    )
}

/// Perform a backup of stable storage at the given offset and limit
#[inline]
pub fn backup_stable_storage(offset: u64, limit: usize) -> ByteBuf {
    let mut bytes = vec![0; limit];
    ic_cdk::api::stable::stable64_read(offset, &mut bytes);
    ByteBuf::from(bytes)
}

/// Initialize the stable storage with the given length
#[inline]
pub fn init_stable_storage(len: u64) {
    let page_count = len / WASM_PAGE_SIZE_IN_BYTES as u64 + 1;
    let current = ic_cdk::api::stable::stable64_size();
    if page_count > current {
        info!("Growing stable storage from {} to {}", current, page_count);
        ic_cdk::api::stable::stable64_grow(page_count).unwrap();
    }
}

/// Restore the stable storage
#[inline]
pub fn restore_stable_storage(offset: u64, bytes: ByteBuf) {
    ic_cdk::api::stable::stable64_write(offset, &bytes.into_vec());
}

/// Restore the stable storage from a compressed array of byte buffers
#[inline]
pub fn restore_stable_storage_compressed(mut offset: u64, compressed_bytes_vec: Vec<ByteBuf>) {
    let mut read_buffer = vec![];
    for bytes in compressed_bytes_vec.into_iter() {
        flate2::read::GzDecoder::new(&bytes.into_vec()[..])
            .read_to_end(&mut read_buffer)
            .unwrap();
        ic_cdk::api::stable::stable64_write(offset, &read_buffer);
        offset += read_buffer.len() as u64;
        read_buffer.clear();
    }
}

/// Set the flag that skips saving the stable storage on next upgrade
#[inline]
pub fn set_restore_from_stable_storage(flag: bool) {
    TRANSIENT.with(|t| t.borrow_mut().skip_next_save = flag);
}

/// v1 implementation for stable storage
pub mod v1 {
    use dscvr_interface::Interface;

    use super::*;

    /// Serialize using v1 layout into canister stable storage
    #[inline]
    pub fn save<T>(interface: &dyn Interface, t: &T) -> Result<(), Error>
    where
        T: serde::Serialize,
    {
        super::super::v1::save(interface, &mut StableWriter::default(), t)
    }

    /// Deserialize using v1 layout into canister stable storage
    pub fn restore<T>(system: &dyn Interface) -> Result<T, Error>
    where
        T: for<'a> serde::Deserialize<'a>,
    {
        let (header, transient, t) =
            super::super::v1::restore(system, &mut StableReader::default())?;
        HEADER.with(|h| *h.borrow_mut() = header);
        TRANSIENT.with(|t| *t.borrow_mut() = transient);
        Ok(t)
    }
}

/// v2 implementation for stable storage
pub mod v2 {
    use dscvr_interface::Interface;

    use crate::data_format::DataFormatType;

    use super::*;

    /// Serialize using v2 layout into canister stable storage
    #[inline]
    pub fn save<T>(
        interface: &dyn Interface,
        t: &T,
        format: DataFormatType,
        version: u64,
    ) -> Result<(), Error>
    where
        T: serde::Serialize,
    {
        info!("Saving using {:?}", format);

        let mut header = HEADER.with(|h| h.borrow().clone());
        header.content_format = format;
        header.content_schema_version = version;

        TRANSIENT.with(|transient| {
            super::super::v2::save(
                interface,
                &mut StableWriter::default(),
                t,
                header,
                &transient.borrow(),
            )
        })
    }

    /// Deserialize using v2 layout into canister stable storage
    pub fn restore<T>(system: &dyn Interface) -> Result<T, Error>
    where
        for<'a> T: serde::Deserialize<'a>,
    {
        let (header, transient, t) =
            super::super::v2::restore(system, &mut StableReader::default())?;
        HEADER.with(|h| *h.borrow_mut() = header);
        TRANSIENT.with(|t| *t.borrow_mut() = transient);
        Ok(t)
    }
}

/// Temporary implementation for transitioning between v1 and v2
pub mod v1_v2 {
    use dscvr_interface::Interface;

    use super::*;

    /// Try restoring via v2 otherwise fallback to v1
    pub fn restore<T>(system: &dyn Interface) -> Result<T, Error>
    where
        for<'a> T: serde::Deserialize<'a>,
    {
        if let Ok(t) = v2::restore(system) {
            info!("Restored using v2");
            return Ok(t);
        }
        info!("v2 restore failed, falling back to v1");
        v1::restore::<T>(system)
    }
}

/// Macro that defines the canister methods to interact with stable storage
/// This is a macro to allow use of guards.
///
/// Note: We don't want these logged in the TX log in case this mechanism is
/// ever used in production, so we use the dscvr_cdk_macros crate to use
/// the ic-cdk macros directly.
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! define_common_stable_storage_interface {
    () => {
        #[cfg(target_arch = "wasm32")]
        #[dscvr_cdk_macros::query(guard = "is_backup_service")]
        fn stable_storage_info(
            _ctx: crate::canister_context::ImmutableContext,
        ) -> ($crate::header::Header, $crate::transient::Transient) {
            $crate::interface::stable_storage_info()
        }

        #[cfg(target_arch = "wasm32")]
        #[dscvr_cdk_macros::query(guard = "is_backup_service")]
        fn backup_stable_storage(
            _ctx: crate::canister_context::ImmutableContext,
            offset: u64,
            limit: usize,
        ) -> serde_bytes::ByteBuf {
            $crate::interface::backup_stable_storage(offset, limit)
        }

        #[cfg(target_arch = "wasm32")]
        #[dscvr_cdk_macros::update(guard = "is_restore_service", skip_tx_log = true)]
        fn init_stable_storage(_ctx: crate::canister_context::MutableContext, len: u64) {
            $crate::interface::init_stable_storage(len);
        }

        #[cfg(target_arch = "wasm32")]
        #[dscvr_cdk_macros::update(guard = "is_restore_service", skip_tx_log = true)]
        fn restore_stable_storage(
            _ctx: crate::canister_context::MutableContext,
            offset: u64,
            bytes: serde_bytes::ByteBuf,
        ) {
            $crate::interface::restore_stable_storage(offset, bytes);
        }

        #[cfg(target_arch = "wasm32")]
        #[dscvr_cdk_macros::update(guard = "is_restore_service", skip_tx_log = true)]
        fn restore_stable_storage_compressed(
            _ctx: crate::canister_context::MutableContext,
            offset: u64,
            compressed_bytes_vec: Vec<serde_bytes::ByteBuf>,
        ) {
            $crate::interface::restore_stable_storage_compressed(offset, compressed_bytes_vec);
        }

        #[cfg(target_arch = "wasm32")]
        #[dscvr_cdk_macros::update(guard = "is_restore_service", skip_tx_log = true)]
        fn set_restore_from_stable_storage(
            _ctx: crate::canister_context::MutableContext,
            flag: bool,
        ) {
            $crate::interface::set_restore_from_stable_storage(flag);
        }
    };
}
