//! Configuration for dscvr.json
mod allocate;
mod persist;
mod provision;

use crate::canister_init_arguments::ControllerType;
use instrumented_error::{IntoInstrumentedError, IntoInstrumentedResult};
use std::collections::hash_map::Entry;

pub use crate::prelude::*;
use crate::schema::dfx::ControllerIdentityMap;
use crate::schema::dscvr::DSCVRGenerationError::MissingElement;
use crate::schema::{
    get_config, DEFAULT_DSCVR_CONFIG_PATH, LOCAL_DSCVR_CONFIG_PATH, LOCAL_NETWORK_NAME,
    PRODUCTION_NETWORK_NAME,
};

pub(super) type Error = DSCVRGenerationError;

pub(crate) const NAME_DELIMITER: &str = ":";

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum DSCVRGenerationError {
    #[error("Error registering available canister.  Cannot find match for {0}")]
    CanisterRegistrationError(String),
    #[error("{0} is missing from the configuration")]
    MissingElement(String),
    #[error("Cannot find {0}.{1}.available_instances to {2}")]
    NoAvailableCanisterInstances(String, String, String),
    #[error("{0}")]
    ProvisionError(String),
}

/// Configuration file for multi-canister support.
///
/// An example document layout is
/// ```json
/// {
///     canisters: {
///         canister_name: {
///
///         }
///     },
///     controller_groups: {
///
///     }
/// }
/// ```.
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct DSCVRConfig {
    /// Stores the canister configurations, on a per name basis.
    pub canisters: HashMap<String, Canister>,
    /// Reusable controller groups stored by name.
    /// Groups can be assigned to canisters on a per-network level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_groups: Option<HashMap<String, ControllerGroup>>,
}

impl DSCVRConfig {
    /// Try to generate config from file for a specified network.
    ///
    /// If the network is `local`, it will use `dscvr.local.json`.
    ///
    /// All other networks use `dscvr.json`.
    ///
    #[tracing::instrument]
    pub fn try_new(network: &str) -> Result<Self> {
        if network == LOCAL_NETWORK_NAME {
            Self::get_or_generate_local()
        } else {
            get_config(Path::new(DEFAULT_DSCVR_CONFIG_PATH))
        }
    }

    pub fn merge_local(base_config: &mut Self) -> Result<()> {
        let mut local_config = Self::get_or_generate_local()?;
        for (can_name, canister) in local_config.canisters.iter_mut() {
            match base_config.canisters.entry(can_name.clone()) {
                Entry::Occupied(mut entrant) => {
                    if let Some(network) = canister.networks.remove(LOCAL_NETWORK_NAME) {
                        entrant
                            .get_mut()
                            .networks
                            .insert(LOCAL_NETWORK_NAME.to_string(), network);
                    }
                }
                Entry::Vacant(e) => {
                    e.insert(canister.clone());
                }
            };
        }

        Ok(())
    }

    /// Helper function to copy the current production canister
    /// instances to another network.  Useful for mimicking prod
    /// to local | staging.
    fn copy_production_instances_to_network(&mut self, network_name: Option<&str>) {
        let mut available;
        let mut provisioned;
        let copy_to_all = network_name.is_none();
        for canister in self.canisters.values_mut() {
            available = canister
                .networks
                .get(PRODUCTION_NETWORK_NAME)
                .and_then(|canister_network| canister_network.available_instances.clone());
            provisioned = canister
                .networks
                .get(PRODUCTION_NETWORK_NAME)
                .and_then(|canister_network| canister_network.provisioned_instances.clone());
            for (name, network) in canister.networks.iter_mut() {
                if copy_to_all || network_name.as_ref().unwrap() == name {
                    network.available_instances = available.clone();
                    network.provisioned_instances = provisioned.clone();
                }
            }
        }
    }

    /// Checks if `./dscvr.local.json` exists.  If the file
    /// does not already exist, it will be created from the
    /// current configuration.
    ///
    /// Generally meant to be used as a setup method
    fn get_or_generate_local() -> Result<DSCVRConfig> {
        let path = Path::new(LOCAL_DSCVR_CONFIG_PATH);
        if !path.exists() {
            let mut config = get_config::<Self>(Path::new(DEFAULT_DSCVR_CONFIG_PATH))?;
            config.copy_production_instances_to_network(Some(LOCAL_NETWORK_NAME));
            config.write_config(LOCAL_NETWORK_NAME)
        } else {
            get_config(path)
        }
    }

    // pub fn get_all_available_instances(&self, canister: &str, network: &str) -> Option<Vec<CanisterInstance>> {
    //     self.get_canister_network(canister, network)?.get_available_instances()
    // }
    //
    pub fn get_all_provisioned_instances(
        &self,
        canister: &str,
        network: &str,
    ) -> Option<Vec<CanisterInstance>> {
        self.get_canister_network(canister, network)?
            .get_provisioned_instances()
    }

    pub fn get_all_instances(
        &self,
        canister: &str,
        network: &str,
    ) -> Option<Vec<CanisterInstance>> {
        self.get_canister_network(canister, network)
            .map(|cn| cn.get_all_instances())
    }

    pub fn get_canister(&self, canister_name: &str) -> Option<&Canister> {
        self.canisters.get(canister_name)
    }

    pub fn get_canister_network(
        &self,
        canister_name: &str,
        network: &str,
    ) -> Option<&CanisterNetwork> {
        self.get_canister(canister_name)?.networks.get(network)
    }

    pub fn get_controller(
        &self,
        canister_name: &str,
        network: &str,
        controller: ControllerType,
    ) -> Option<&IdentityFromFile> {
        self.get_all_controllers_for_canister_network(canister_name, network)
            .ok()?
            .controllers
            .get(&controller)
    }

    pub fn get_all_controllers_for_canister_network(
        &self,
        canister_name: &str,
        network: &str,
    ) -> Result<&ControllerGroup> {
        let canister = self
            .get_canister(canister_name)
            .ok_or_else(|| format!("{canister_name} not found").into_instrumented_error())?;
        let controller_group = canister
            .networks
            .get(network)
            .ok_or_else(|| {
                format!("Network {network} does not exist for canister {canister_name}")
                    .into_instrumented_error()
            })?
            .controllers
            .as_ref()
            .ok_or_else(|| {
                format!("Controllers group not listed on {canister_name}:{network}")
                    .into_instrumented_error()
            })?;
        self.controller_groups
            .as_ref()
            .ok_or_else(|| {
                String::from("No controller groups listed in document root")
                    .into_instrumented_error()
            })?
            .get(controller_group)
            .ok_or_else(|| {
                format!("No ControllerGroup found for {canister_name}:{network}:{controller_group}")
            })
            .into_instrumented_result()
    }

    pub(super) fn get_canister_for_network_mut(
        &mut self,
        canister_name: &str,
        network: &str,
    ) -> std::result::Result<&mut CanisterNetwork, Error> {
        self.canisters
            .get_mut(canister_name)
            .ok_or_else(|| MissingElement(canister_name.to_string()))?
            .networks
            .get_mut(network)
            .ok_or_else(|| MissingElement(format!("{canister_name}.{network} ")))
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Canister {
    /// List of networks and their configuration settings
    /// for this canister.  Maps loosely to dfx.json providers.
    #[serde(flatten)]
    pub networks: HashMap<String, CanisterNetwork>,
    /// Path to this canisters candid (.did) file
    pub candid: String,
    /// Path to canister's wasm module
    pub wasm: String,
    /// Canister build specific instructions
    pub build: String,
    /// Maps to custom dscvr field used in dfx.json
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_init_params: Option<bool>,
    /// Maps to custom dscvr field used in dfx.json
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_stable_storage_backup_restore: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct CanisterNetwork {
    /// Provider URL
    pub provider: String,
    /// Name of the corresponding `ControllerGroup` (if any)
    /// for this network.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controllers: Option<String>,
    /// List of instances that have been created and have this canisters
    /// wasm module installed on this network.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provisioned_instances: Option<Vec<CanisterInstance>>,
    /// List of instances that have been created _but do not yet_ have the
    /// corresponding wasm module installed on this network.
    /// These are available to be provisioned.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_instances: Option<Vec<CanisterInstance>>,
    /// Wallet id to use with this canister (if applicable)
    /// We can move this to instance level if we desire.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
}

impl CanisterNetwork {
    fn search_provisioned(
        &self,
        instance_name: Option<&String>,
        instance_id: Option<&String>,
    ) -> Option<&CanisterInstance> {
        if let Some(instance_name) = instance_name {
            self.provisioned_instances
                .as_ref()
                .and_then(|instances| instances.iter().find(|i| i.name == *instance_name))
        } else {
            let instance_id = instance_id?;
            self.provisioned_instances.as_ref().and_then(|instances| {
                instances
                    .iter()
                    .find(|i| i.id.as_ref().unwrap_or(&Default::default()) == instance_id)
            })
        }
    }

    fn search_available(
        &self,
        instance_name: Option<&String>,
        instance_id: Option<&String>,
    ) -> Option<&CanisterInstance> {
        if let Some(instance_name) = instance_name {
            self.available_instances
                .as_ref()
                .and_then(|instances| instances.iter().find(|i| i.name == *instance_name))
        } else {
            let instance_id = instance_id?;
            self.available_instances.as_ref().and_then(|instances| {
                instances
                    .iter()
                    .find(|i| i.id.as_ref().unwrap_or(&Default::default()) == instance_id)
            })
        }
    }

    pub fn find_instance(
        &self,
        instance_name: Option<&String>,
        instance_id: Option<&String>,
    ) -> Option<&CanisterInstance> {
        if let Some(instance) = self.search_provisioned(instance_name, instance_id) {
            Some(instance)
        } else {
            self.search_available(instance_name, instance_id)
        }
    }

    /// Gets all instances for the current CanisterNetwork.
    /// This includes both `provisioned_instances` of a canister
    /// (where a wasm has been installed) and `available_instances`
    /// of a canister (where the canister is created, but no
    /// wasm has yet been installed).
    ///
    pub fn get_all_instances(&self) -> Vec<CanisterInstance> {
        let mut provisioned = self.provisioned_instances.clone().unwrap_or_default();
        let mut available = self.available_instances.clone().unwrap_or_default();
        provisioned.append(&mut available);
        provisioned
    }

    /// Gets only `available_instances` of the canister
    /// for the current CanisterNetwork.
    ///
    pub fn get_available_instances(&self) -> Option<Vec<CanisterInstance>> {
        self.available_instances.clone()
    }

    /// Gets only `provisioned_instances` of the canister
    /// for the current CanisterNetwork.
    ///
    pub fn get_provisioned_instances(&self) -> Option<Vec<CanisterInstance>> {
        self.provisioned_instances.clone()
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct ControllerGroup {
    #[serde(flatten)]
    pub controllers: ControllerIdentityMap,
}

#[derive(Debug, Default, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct CanisterInstance {
    /// Plaintext name of the instance
    /// Maps to the `canister_name` in dfx.json
    pub name: String,
    /// Canister ID of the instance.  Corresponds to the ID
    /// for the canister found in `canister_ids.json`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::schema::{DEFAULT_CANISTER_IDS_PATH, DEFAULT_DFX_CONFIG_PATH};
    use std::io::{BufReader, BufWriter};
    use std::str::FromStr;

    const LOCAL_PROVIDER: &str = "http://localhost:8000";
    const IC_PROVIDER: &str = "https://ic0.app";
    const STAGING_PROVIDER: &str = "https://replica1.stg.dscvr.cloud";

    fn cleanup() {
        let path = Path::new(LOCAL_DSCVR_CONFIG_PATH);
        if path.exists() {
            std::fs::remove_file(path).expect("Successfully removed");
        }

        let path = Path::new(DEFAULT_DSCVR_CONFIG_PATH);
        if path.exists() {
            std::fs::remove_file(path).expect("Successfully removed");
        }

        let path = Path::new(DEFAULT_DFX_CONFIG_PATH);
        if path.exists() {
            std::fs::remove_file(path).expect("Successfully removed");
        }

        let path = Path::new(DEFAULT_CANISTER_IDS_PATH);
        if path.exists() {
            std::fs::remove_file(path).expect("Successfully removed");
        }
    }

    #[test]
    #[ignore]
    fn test_json() {
        // {
        //   "dscvr-event-router": {
        //     "ic": "ccmhu-fqaaa-aaaab-qahoa-cai",
        //     "staging": "ryjl3-tyaaa-aaaaa-aaaba-cai"
        //   },
        //   "society_rs": {
        //     "ic": "h2bch-3yaaa-aaaab-qaama-cai",
        //     "staging": "rrkah-fqaaa-aaaaa-aaaaq-cai"
        //   },
        //   "society_rs_assets": {
        //     "ic": "h5aet-waaaa-aaaab-qaamq-cai"
        //   },
        //   "stable-storage-test": {
        //     "staging": "r7inp-6aaaa-aaaaa-aaabq-cai"
        //   }
        // }

        let mut prod_group = ControllerGroup {
            controllers: Default::default(),
        };
        prod_group.controllers.insert(
            ControllerType::Backup,
            IdentityFromFile::from_str("./keys/ic-service-account-backup.pem").unwrap(),
        );
        prod_group.controllers.insert(
            ControllerType::TxLogConsumer,
            IdentityFromFile::from_str("./keys/prod-tx-log-consumer.pem").unwrap(),
        );

        let mut local_group = ControllerGroup {
            controllers: Default::default(),
        };
        local_group.controllers.insert(
            ControllerType::Backup,
            IdentityFromFile::from_str("./keys/service-account-backup.pem").unwrap(),
        );
        local_group.controllers.insert(
            ControllerType::Restore,
            IdentityFromFile::from_str("./keys/service-account-restore.pem").unwrap(),
        );
        local_group.controllers.insert(
            ControllerType::TxLogConsumer,
            IdentityFromFile::from_str("./keys/service-account-tx-log-consumer.pem").unwrap(),
        );
        local_group.controllers.insert(
            ControllerType::Owner,
            IdentityFromFile::from_str("./keys/local-default.pem").unwrap(),
        );

        let mut staging_group = ControllerGroup {
            controllers: Default::default(),
        };
        staging_group.controllers.insert(
            ControllerType::Backup,
            IdentityFromFile::from_str("./keys/staging-backup.pem").unwrap(),
        );
        staging_group.controllers.insert(
            ControllerType::Restore,
            IdentityFromFile::from_str("./keys/staging-restore.pem").unwrap(),
        );
        staging_group.controllers.insert(
            ControllerType::Owner,
            IdentityFromFile::from_str("./keys/staging-create.pem").unwrap(),
        );
        staging_group.controllers.insert(
            ControllerType::TxLogConsumer,
            IdentityFromFile::from_str("./keys/staging-tx-log-consumer.pem").unwrap(),
        );

        let controller_groups = HashMap::from([
            ("prod".to_string(), prod_group.clone()),
            ("staging".to_string(), staging_group.clone()),
            ("local".to_string(), local_group.clone()),
        ]);

        let mut dscvr_config = DSCVRConfig {
            canisters: Default::default(),
            controller_groups: None,
        };

        let mut society_rs = Canister {
            networks: Default::default(),
            candid: "crates/canisters/society_rs/society.did".to_string(),
            wasm: "./export/wasms/society_rs.wasm.gz".to_string(),
            build: "./build-scripts/dscvr-cli.sh build society_rs".to_string(),
            supports_init_params: Some(true),
            supports_stable_storage_backup_restore: Some(true),
        };

        let society_rs_ic = CanisterNetwork {
            provider: IC_PROVIDER.to_string(),
            controllers: Some("prod".to_string()),
            provisioned_instances: Some(vec![CanisterInstance {
                name: "society_rs".to_string(),
                id: Some("h2bch-3yaaa-aaaab-qaama-cai".to_string()),
            }]),
            available_instances: None,
            wallet: Some("g6mnv-cyaaa-aaaab-qaaka-cai".to_string()),
        };

        let society_rs_staging = CanisterNetwork {
            provider: STAGING_PROVIDER.to_string(),
            controllers: Some("staging".to_string()),
            provisioned_instances: Some(vec![CanisterInstance {
                name: "society_rs".to_string(),
                id: Some("rrkah-fqaaa-aaaaa-aaaaq-cai".to_string()),
            }]),
            available_instances: None,
            wallet: None,
        };

        let society_rs_local = CanisterNetwork {
            provider: LOCAL_PROVIDER.to_string(),
            controllers: Some("local".to_string()),
            provisioned_instances: None,
            available_instances: None,
            wallet: None,
        };

        society_rs.networks.insert("ic".to_string(), society_rs_ic);
        society_rs
            .networks
            .insert("staging".to_string(), society_rs_staging);
        society_rs
            .networks
            .insert("local".to_string(), society_rs_local);

        let mut event_router = Canister {
            networks: Default::default(),
            candid: "crates/canisters/dscvr-event-router/dscvr-event-router.did".to_string(),
            wasm: "./export/wasms/dscvr-event-router.wasm.gz".to_string(),
            build: "./build-scripts/dscvr-cli.sh build dscvr-event-router".to_string(),
            supports_init_params: Some(true),
            supports_stable_storage_backup_restore: None,
        };

        let event_router_ic = CanisterNetwork {
            provider: IC_PROVIDER.to_string(),
            controllers: Some("prod".to_string()),
            provisioned_instances: Some(vec![CanisterInstance {
                name: "dscvr-event-router".to_string(),
                id: Some("ccmhu-fqaaa-aaaab-qahoa-cai".to_string()),
            }]),
            available_instances: None,
            wallet: Some("g6mnv-cyaaa-aaaab-qaaka-cai".to_string()),
        };

        let event_router_staging = CanisterNetwork {
            provider: STAGING_PROVIDER.to_string(),
            controllers: Some("staging".to_string()),
            provisioned_instances: Some(vec![CanisterInstance {
                name: "dscvr-event-router".to_string(),
                id: Some("ryjl3-tyaaa-aaaaa-aaaba-cai".to_string()),
            }]),
            available_instances: None,
            wallet: None,
        };

        let event_router_local = CanisterNetwork {
            provider: LOCAL_PROVIDER.to_string(),
            controllers: Some("local".to_string()),
            provisioned_instances: None,
            available_instances: None,
            wallet: None,
        };

        event_router
            .networks
            .insert("ic".to_string(), event_router_ic);
        event_router
            .networks
            .insert("staging".to_string(), event_router_staging);
        event_router
            .networks
            .insert("local".to_string(), event_router_local);

        dscvr_config.controller_groups = Some(controller_groups);
        dscvr_config
            .canisters
            .insert("society_rs".to_string(), society_rs);
        dscvr_config
            .canisters
            .insert("dscvr-event-router".to_string(), event_router);

        let path = "./dscvr.json";
        let writer = BufWriter::new(std::fs::File::create(path).expect("Able to create file"));
        serde_json::to_writer(writer, &dscvr_config).expect("File written");

        let config = serde_json::from_reader::<_, DSCVRConfig>(BufReader::new(
            std::fs::File::open(path).expect("File exists"),
        ));
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.controller_groups, dscvr_config.controller_groups);
        assert_eq!(config.canisters, dscvr_config.canisters);

        let dfx =
            crate::schema::dfx::DfxConfig::try_from_dscvr_for_network(dscvr_config, "ic").unwrap();
        crate::schema::write_config("./dfx.json", &dfx).unwrap();
    }

    #[test]
    #[ignore]
    fn test_dscvr_local() {
        let path = Path::new(LOCAL_DSCVR_CONFIG_PATH);
        if path.exists() {
            std::fs::remove_file(path).expect("Successfully removed");
        }
        let dscvr_config = DSCVRConfig::try_new(LOCAL_NETWORK_NAME).expect("Got some file back");
        assert!(dscvr_config.controller_groups.is_some());
        for canister in dscvr_config.canisters.values() {
            assert_eq!(canister.networks.len(), 1);
            assert!(canister.networks.contains_key(LOCAL_NETWORK_NAME));
            assert!(canister
                .networks
                .get(LOCAL_NETWORK_NAME)
                .unwrap()
                .provisioned_instances
                .is_some());
        }
    }

    #[test]
    #[ignore]
    fn test_merge_path() {
        let mut dscvr_config = DSCVRConfig::try_new(PRODUCTION_NETWORK_NAME).expect("File exists");
        for canister in dscvr_config.canisters.values() {
            assert!(canister
                .networks
                .get(LOCAL_NETWORK_NAME)
                .unwrap()
                .provisioned_instances
                .is_none());
        }

        DSCVRConfig::merge_local(&mut dscvr_config).expect("Able to merge");
        for canister in dscvr_config.canisters.values() {
            assert!(canister
                .networks
                .get(LOCAL_NETWORK_NAME)
                .unwrap()
                .provisioned_instances
                .is_some());
        }

        cleanup()
    }
}
