use super::*;
use crate::schema::{
    write_config, DEFAULT_DSCVR_CONFIG_PATH, LOCAL_DSCVR_CONFIG_PATH, LOCAL_NETWORK_NAME,
};

impl DSCVRConfig {
    fn generate_default_config(&self) -> Self {
        let mut other_self = self.clone();
        let mut canisters_to_remove = vec![];
        for (canister_name, canister) in other_self.canisters.iter_mut() {
            if canister.networks.len() == 1 && canister.networks.contains_key(LOCAL_NETWORK_NAME) {
                canisters_to_remove.push(canister_name.clone())
            }

            for (network_name, network) in canister.networks.iter_mut() {
                if network_name == LOCAL_NETWORK_NAME {
                    network.available_instances = None;
                    network.provisioned_instances = None;
                }
            }
        }

        for canister_to_remove in &canisters_to_remove {
            other_self.canisters.remove(canister_to_remove);
        }

        other_self
    }

    fn generate_local_config(&self) -> Self {
        let mut other_self = self.clone();

        for canister in other_self.canisters.values_mut() {
            canister.networks.retain(|k, _| k == LOCAL_NETWORK_NAME);
        }

        other_self
    }

    /// Helper method to persist this config to file.
    ///
    /// When persisting for `local` network(s) we strip out
    /// any instances from other networks.  Since `dscvr.local.json`
    /// is not checked in, we want to make sure we don't corrupt that
    /// file with stale canister data.
    ///
    /// When persisting to any network other than local, we strip out
    /// local canister instances (since it may cause conflicts between
    /// developers on check-in).
    ///
    /// Use this method whenever you want to persist this config
    /// to file.
    pub(crate) fn write_config(&self, network: &str) -> Result<Self> {
        if network == LOCAL_NETWORK_NAME {
            let config_to_write = self.generate_local_config();
            write_config(LOCAL_DSCVR_CONFIG_PATH, &config_to_write)?;
            Ok(config_to_write)
        } else {
            let config_to_write = self.generate_default_config();
            write_config(DEFAULT_DSCVR_CONFIG_PATH, &config_to_write)?;
            Ok(config_to_write)
        }
    }
}
