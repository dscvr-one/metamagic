#![deny(missing_docs)]

//! Functionality for registering canister lifecycle and methods for use
// with the dscvr canister mirror

use std::collections::HashMap;

/// Define the types that allow exporting canister methods
#[macro_export]
#[allow(clippy::crate_in_macro_def)]
macro_rules! define_canister_exports {
    () => {
        pub mod canister_exports {
            /// Aliased type for a canister query method
            pub type Method = fn(crate::canister_context::ImmutableContext<'_>, &[u8]) -> Result<Vec<u8>, String>;
            /// Aliased type for a canister update method
            pub type UpdateMethod = fn(
                crate::canister_context::MutableContext<'_>,
                &[u8],
                crate::canister_context::UpdateContext<'_>,
            ) -> Result<Vec<u8>, String>;
            /// Aliased type for a cansiter init method
            pub type Init =
                fn(crate::canister_context::MutableContext<'_>, &[u8], crate::canister_context::UpdateContext<'_>);
            /// Aliased type for a canister post upgrade and pre upgrade method
            pub type Lifecycle =
                fn(crate::canister_context::MutableContext<'_>, crate::canister_context::UpdateContext<'_>);

            /// A canister query method registration
            pub type MethodRegistration = (&'static str, Method);
            /// A canister update method registration
            pub type UpdateMethodRegistration = (&'static str, UpdateMethod);
            /// Registration for init
            pub type InitRegistration = (&'static str, Init);
            /// Registration for pre and post upgrade
            pub type LifecycleRegistration = (&'static str, Lifecycle);

            /// Distributed slice for canister update methods
            #[linkme::distributed_slice]
            pub static UPDATE_METHODS: [UpdateMethodRegistration] = [..];

            /// Distributed slice for canister update methods
            #[linkme::distributed_slice]
            pub static QUERY_METHODS: [MethodRegistration] = [..];

            /// Distributed slice for canister post upgrade
            #[linkme::distributed_slice]
            pub static POST_UPGRADE: [LifecycleRegistration] = [..];

            /// Distributed slice for canister pre upgrade
            #[linkme::distributed_slice]
            pub static PRE_UPGRADE: [LifecycleRegistration] = [..];

            /// Distributed slice for canister init
            #[linkme::distributed_slice]
            pub static INIT: [InitRegistration] = [..];

            pub fn definition(primary: bool) -> $crate::CanisterDefinition<crate::State> {
                $crate::CanisterDefinition::new(
                    &UPDATE_METHODS,
                    &QUERY_METHODS,
                    &INIT,
                    &POST_UPGRADE,
                    &PRE_UPGRADE,
                    primary,
                )
            }
        }
    };
}

/// Aliased type for a canister query method
pub type CanisterMethod<State> =
    fn(dscvr_canister_context::ImmutableContext<'_, State>, &[u8]) -> Result<Vec<u8>, String>;
/// Aliased type for a canister update method
pub type CanisterUpdateMethod<State> = fn(
    dscvr_canister_context::MutableContext<'_, State>,
    &[u8],
    dscvr_canister_context::UpdateContext<'_>,
) -> Result<Vec<u8>, String>;
/// Aliased type for a cansiter init method
pub type CanisterInitMethod<State> =
    fn(dscvr_canister_context::MutableContext<'_, State>, &[u8], dscvr_canister_context::UpdateContext<'_>);
/// Aliased type for a cansiter lifecycle method
pub type CanisterLifecycleMethod<State> =
    fn(dscvr_canister_context::MutableContext<'_, State>, dscvr_canister_context::UpdateContext<'_>);

/// A single canister registration
pub struct CanisterDefinition<State> {
    /// Hashmap of candid name to the update method
    pub update_methods: HashMap<String, CanisterUpdateMethod<State>>,
    /// Hashmap of candid name to the query method
    pub query_methods: HashMap<String, CanisterMethod<State>>,
    /// Init method
    pub init_method: CanisterInitMethod<State>,
    /// Pre upgrade method
    pub pre_upgrade: CanisterLifecycleMethod<State>,
    /// Post upgrade method
    pub post_upgrade: CanisterLifecycleMethod<State>,
    /// Is this the primary registration
    pub primary: bool,
}

impl<State> CanisterDefinition<State> {
    /// Returns a registration by reading from the registered slices
    pub fn new(
        updates: &[(&'static str, CanisterUpdateMethod<State>)],
        queries: &[(&'static str, CanisterMethod<State>)],
        init: &[(&'static str, CanisterInitMethod<State>)],
        post_upgrade: &[(&'static str, CanisterLifecycleMethod<State>)],
        pre_upgrade: &[(&'static str, CanisterLifecycleMethod<State>)],
        primary: bool,
    ) -> Self {
        let mut update_methods = HashMap::new();
        let mut query_methods = HashMap::new();

        for (name, method) in updates {
            update_methods.insert(name.to_string(), *method);
        }

        for (name, method) in queries {
            query_methods.insert(name.to_string(), *method);
        }

        CanisterDefinition {
            update_methods,
            query_methods,
            init_method: init[0].1,
            post_upgrade: post_upgrade[0].1,
            pre_upgrade: pre_upgrade[0].1,
            primary,
        }
    }
}
