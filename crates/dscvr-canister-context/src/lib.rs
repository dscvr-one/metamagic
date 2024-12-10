#![deny(missing_docs)]

//! Common logic for managing global canister state and context.

use dscvr_interface::Interface;

/// Enum used to describe the sub type of an update.
#[derive(Eq, PartialEq, Debug)]
pub enum UpdateContext<'a> {
    /// Update that runs on the primary and
    /// should be appended to the TxLog
    Primary,
    /// Update that is replayed on the Secondary and
    /// should not be appended to the TxLog and does
    /// not require response validation
    Secondary,
    /// Update that is replayed on the Secondary and
    /// should not be appended to the TxLog but does
    /// require response validation.
    SecondaryWithValidation(&'a [u8]),
}

/// Context that only allows read access to state.
/// Passed as an argument to queries
pub struct ImmutableContext<'a, State> {
    state: &'a State,
    /// The system interface
    system: &'a dyn Interface,
}

impl<'a, State> ImmutableContext<'a, State> {
    /// Read a state with function
    #[inline]
    pub fn read<F: FnOnce(&State) -> R, R>(&self, f: F) -> R {
        f(self.state)
    }

    /// Read a state and system with function
    #[inline]
    pub fn read_with_system<F: FnOnce(&State, &dyn Interface) -> R, R>(&self, f: F) -> R {
        f(self.state, self.system)
    }

    /// Create a new context
    #[inline]
    pub fn new(state: &'a State, system: &'a dyn Interface) -> Self {
        Self { state, system }
    }

    /// Return the system
    #[inline]
    pub fn system(&self) -> &dyn Interface {
        self.system
    }

    /// Return the state
    #[inline]
    pub fn state(&self) -> &State {
        self.state
    }
}

impl<State> Clone for ImmutableContext<'_, State> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            state: self.state,
            system: self.system,
        }
    }
}

/// Context that only allows read or write access to state.
/// Passed as an argument to updates, post-upgrade, pre-upgrade, and init
pub struct MutableContext<'a, State> {
    state: &'a mut State,
    /// The system interface
    system: &'a dyn Interface,
}

impl<'a, State> MutableContext<'a, State> {
    /// Read a state with function
    #[inline]
    pub fn read<F: FnOnce(&State) -> R, R>(&self, f: F) -> R {
        f(self.state)
    }

    /// Read a state and system with function
    #[inline]
    pub fn read_with_system<F: FnOnce(&State, &dyn Interface) -> R, R>(&self, f: F) -> R {
        f(self.state, self.system)
    }

    /// Mutate a state with function
    #[inline]
    pub fn mutate<F: FnOnce(&mut State) -> R, R>(&mut self, f: F) -> R {
        f(self.state)
    }

    /// Mutate a state and system with function
    #[inline]
    pub fn mutate_with_system<F: FnOnce(&mut State, &dyn Interface) -> R, R>(&mut self, f: F) -> R {
        f(self.state, self.system)
    }

    /// Create a new context
    #[inline]
    pub fn new(state: &'a mut State, system: &'a dyn Interface) -> Self {
        Self { state, system }
    }

    /// Return the system
    #[inline]
    pub fn system(&self) -> &dyn Interface {
        self.system
    }

    /// Return the state
    #[inline]
    pub fn state(&self) -> &State {
        self.state
    }

    /// Return the mutable state
    #[inline]
    pub fn state_mut(&mut self) -> &mut State {
        self.state
    }
}

impl<'a, 'b, State> From<&'b MutableContext<'a, State>> for ImmutableContext<'a, State>
where
    'b: 'a,
{
    #[inline]
    fn from(m: &'b MutableContext<'a, State>) -> Self {
        Self {
            state: m.state,
            system: m.system,
        }
    }
}

impl<'a, 'b, State> From<&'b mut MutableContext<'a, State>> for ImmutableContext<'a, State>
where
    'b: 'a,
{
    #[inline]
    fn from(m: &'b mut MutableContext<'a, State>) -> Self {
        Self {
            state: m.state,
            system: m.system,
        }
    }
}

impl<'a, State> From<MutableContext<'a, State>> for ImmutableContext<'a, State> {
    #[inline]
    fn from(m: MutableContext<'a, State>) -> Self {
        Self {
            state: m.state,
            system: m.system,
        }
    }
}

/// Macro to define the global state interface that's used
/// for canisters that supports off-chain canister mirroring.
///
/// Note: This is a macro since generics are not allowed in
/// static instances.
///
#[macro_export]
macro_rules! define_common_state_interface {
    ($state: ty) => {
        pub mod canister_context {
            use super::*;
            pub use $crate::UpdateContext;

            pub type StateType = $state;

            pub type ImmutableContext<'a> = $crate::ImmutableContext<'a, $state>;
            pub type MutableContext<'a> = $crate::MutableContext<'a, $state>;
        }

        #[cfg(target_arch = "wasm32")]
        impl $state {
            thread_local! {
                static STATE: std::cell::RefCell<$state> = std::cell::RefCell::default();
            }

            #[inline]
            pub fn read_state<F: FnOnce(&Self) -> R, R>(f: F) -> R {
                Self::STATE.with(|s| f(&s.borrow()))
            }

            #[inline]
            pub fn mutate_state<F: FnOnce(&mut Self) -> R, R>(f: F) -> R {
                Self::STATE.with(|s| f(&mut s.borrow_mut()))
            }
        }
    };
}

/// Macro to define the global state interface that's used
/// for canisters.
///
/// Note: This is a macro since generics are not allowed in
/// static instances.
///
/// Note: This macro is deprecated
#[macro_export]
macro_rules! define_v1_common_state_interface {
    ($state: ty) => {
        pub mod canister_context {
            use super::*;
            pub use $crate::UpdateContext;

            pub type StateType = $state;

            pub type ImmutableContext<'a> = $crate::ImmutableContext<'a, $state>;
            pub type MutableContext<'a> = $crate::MutableContext<'a, $state>;
        }

        #[cfg(not(target_arch = "wasm32"))]
        lazy_static::lazy_static! {
            static ref STATE: std::sync::Arc<std::sync::RwLock<$state>> = {
                std::sync::Arc::new(std::sync::RwLock::new(std::default::Default::default()))
            };
        }

        #[cfg(not(target_arch = "wasm32"))]
        impl $state {
            #[inline]
            pub fn read_state<F: FnOnce(&Self) -> R, R>(f: F) -> R {
                let state = STATE.read().expect("read lock");
                f(&state)
            }
            #[inline]
            pub fn mutate_state<F: FnOnce(&mut Self) -> R, R>(f: F) -> R {
                let mut state = STATE.write().expect("write lock");
                f(&mut state)
            }
        }

        #[cfg(target_arch = "wasm32")]
        impl $state {
            thread_local! {
                static STATE: std::cell::RefCell<$state> = std::cell::RefCell::default();
            }

            #[inline]
            pub fn read_state<F: FnOnce(&Self) -> R, R>(f: F) -> R {
                Self::STATE.with(|s| f(&s.borrow()))
            }

            #[inline]
            pub fn mutate_state<F: FnOnce(&mut Self) -> R, R>(f: F) -> R {
                Self::STATE.with(|s| f(&mut s.borrow_mut()))
            }
        }
    };
}
