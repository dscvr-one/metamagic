//! Implements ref-counted principals to reduce memory usage.
//!
//! Background:
//!
//! Each principal uses 30 bytes in memory:
//!     - 29 fixed-size slice for data
//!     - 1 byte for the length
//!     - Total: 30 bytes
//!
//! Reusing principals via a ref-counter reduces to a one-time memory cost:
//!     - 2 usize for weak and strong rc tracking (8 bytes in 32-bit and 16 bytes in 64-bit)
//!     - 1 pointer (4 bytes in 32-bit and 8 bytes in 64-bit)
//!     - Total: 12 bytes for 32-bit, 24 for 64-bit
//!
//! There's a small instruction cost to perform the lookup of the principal to the ref-counted
//! principal. This can be mitigated by performing the lookup just prior to insertion into the
//! store.
use candid::{CandidType, Deserialize, Principal};
use rustc_hash::FxHashMap;
use serde::Serialize;
use std::{borrow::Borrow, cell::RefCell};

thread_local! {
    pub static MAP: RefCell<FxHashMap<RcPrincipal, RcPrincipal>> = RefCell::default();
}

/// A unit-struct that wraps aroudn a ref-counted implementation to facilitate
/// implementing foreign traits such as `Hash`, `Eq`, `Display`, etc.
#[cfg(target_arch = "wasm32")]
type InnerType = std::rc::Rc<Principal>;
#[cfg(not(target_arch = "wasm32"))]
type InnerType = std::sync::Arc<Principal>;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(deepsize::DeepSizeOf))]
pub struct RcPrincipal(InnerType);

impl RcPrincipal {
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    #[inline]
    pub fn new(p: Principal) -> RcPrincipal {
        RcPrincipal::get(&p)
    }

    #[inline]
    pub fn inner(&self) -> &Principal {
        &self.0
    }

    pub fn get(p: &Principal) -> RcPrincipal {
        MAP.with(|map| {
            if let Some(principal) = map.borrow().get(p) {
                return principal.clone();
            }

            let rc_p = RcPrincipal(InnerType::new(*p));
            map.borrow_mut().insert(rc_p.clone(), rc_p.clone());
            rc_p
        })
    }
}

// Passhtru implementation of Display
impl std::fmt::Display for RcPrincipal {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

// Implementation of Deserialize, which resolves to the ref-counted principal.
impl<'de> Deserialize<'de> for RcPrincipal {
    #[inline]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let p = Principal::deserialize(deserializer)?;
        Ok(RcPrincipal::get(&p))
    }
}

// Passhtru implementation of Serialize
impl Serialize for RcPrincipal {
    #[inline]
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

// Passhtru implementation of CandidType
impl CandidType for RcPrincipal {
    #[inline]
    fn _ty() -> candid::types::Type {
        Principal::_ty()
    }

    #[inline]
    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        self.0.idl_serialize(serializer)
    }
}

impl From<Principal> for RcPrincipal {
    #[inline]
    fn from(p: Principal) -> Self {
        RcPrincipal::get(&p)
    }
}

impl From<&Principal> for RcPrincipal {
    #[inline]
    fn from(p: &Principal) -> Self {
        RcPrincipal::get(p)
    }
}

impl From<RcPrincipal> for Principal {
    #[inline]
    fn from(p: RcPrincipal) -> Self {
        *p.0
    }
}

impl From<&RcPrincipal> for Principal {
    #[inline]
    fn from(p: &RcPrincipal) -> Self {
        *p.0
    }
}

// Borrow implementation.
//
// Allows `RcPrincipal` to be used a key in the hashmap and
// lookup to be performed via `Principal` without conversion to the `RcPrincipal`.
impl Borrow<Principal> for RcPrincipal {
    #[inline]
    fn borrow(&self) -> &Principal {
        &self.0
    }
}
