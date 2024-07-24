//! Configuration from dfx.json
use crate::canister_init_arguments::ControllerType;
use crate::prelude::*;
use crate::schema::dscvr::DSCVRConfig;
use crate::schema::LOCAL_NETWORK_NAME;
use ic_identity_util::IdentityFromFile;
use std::collections::HashMap;
use std::path::Path;

const DFX_VERSION: &str = "0.14.1";

type Error = DfxGenerationError;

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
pub enum DfxGenerationError {
    #[error("Controller Group {0} Specified for Canister {0} not found in DSCVRRoot")]
    ControllerGroupMismatch(String, String),
    #[error(
        "Controller Groups Specified for Canister {0} but no Controller Groups found in DSCVRRoot"
    )]
    ControllerGroupMissing(String),
}

/// Configuration for the canisters. We extend the dfx.json schema and superimpose
/// our own fields that are specific to DSCVR use-cases.
#[derive(Deserialize, Serialize)]
pub struct DfxConfig {
    /// canister_name -> canister config
    pub canisters: HashMap<String, DfxCanister>,
    /// network_name -> network_config
    pub networks: HashMap<String, DfxNetwork>,
    /// The order in which the canisters are setup. This is needed
    /// to have a reproducible staging environment such that each
    /// canister is always assigned the same canister id.
    pub canister_setup_order: Vec<String>,
    /// DFX Version
    pub dfx: String,
}

/// Controller type -> identity map
pub type ControllerIdentityMap = HashMap<ControllerType, IdentityFromFile>;

/// Canister configuration
#[derive(Deserialize, Serialize)]
pub struct DfxCanister {
    /// The name of the canister
    #[serde(skip)]
    pub name: String,
    /// Path to the wasm
    pub wasm: String,
    /// Path to the candid definition
    pub candid: String,
    /// Set to true if the canister can be initialized with parameters (DSCVR)
    #[serde(default = "Default::default")]
    pub supports_init_params: bool,
    /// Set to true if the canister supports backup
    /// and restore via stable storage (DSCVR)
    #[serde(default = "Default::default")]
    pub supports_stable_storage_backup_restore: bool,
    /// Canister network to canister id map defined in canister_ids.json
    #[serde(skip)]
    pub network_id_map: HashMap<String, String>,
    /// network name to wallet id map used for deploying the canister.
    pub wallet: Option<HashMap<String, String>>,
    /// The controllers associated with this canister (DSCVR)
    pub controllers: Option<HashMap<String, ControllerIdentityMap>>,
    /// Build parameters (DSCVR)
    pub build: String,
}

impl DfxCanister {
    /// Return all the controllers for a network
    pub fn get_all_controllers_for_network(
        &self,
        network_name: &str,
    ) -> Option<&ControllerIdentityMap> {
        self.controllers.as_ref()?.get(network_name)
    }

    /// Return the controller for this canister for a specific network.
    pub fn get_controller(
        &self,
        network_name: &str,
        controller_type: &ControllerType,
    ) -> Option<&IdentityFromFile> {
        self.controllers
            .as_ref()?
            .get(network_name)?
            .get(controller_type)
    }
}

/// Network configuration
#[derive(Deserialize, Serialize)]
pub struct DfxNetwork {
    /// The name of the network
    #[serde(skip)]
    pub name: String,
    /// Providers for this network
    pub providers: Option<Vec<String>>,
    /// Bound address
    pub bind: Option<String>,
}

impl DfxConfig {
    /// Create a configuration from a JSON file
    #[tracing::instrument()]
    pub fn new_from_file<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path> + std::fmt::Debug,
    {
        let path = path.as_ref();
        let path_parent = path.parent().expect("parent");
        let f = File::open(path)?;
        let mut val: Self = serde_json::from_reader(f)?;
        val.update_names();

        // create a dummy entry for the local network
        // TODO: get from shared network definition.
        val.networks.insert(
            "local".to_owned(),
            DfxNetwork {
                name: "local".to_owned(),
                providers: None,
                bind: Some("127.0.0.1:8000".to_owned()),
            },
        );

        // initialize identities
        for canister in val.canisters.values_mut() {
            if let Some(networks) = &mut canister.controllers {
                for controllers in networks.values_mut() {
                    for identity in controllers.values_mut() {
                        identity.join_parent(path_parent);
                    }
                }
            }
        }

        val.init_canister_id_map(path)?;

        Ok(val)
    }

    /// Create a configuration from the default JSON file
    pub fn new_from_default_file() -> Result<Self> {
        let dfx_json_path = "dfx.json".to_string();
        tracing::debug!("READ configuration {}", &dfx_json_path);
        Self::new_from_file(&dfx_json_path)
    }

    fn update_names(&mut self) {
        for (k, val) in self.canisters.iter_mut() {
            val.name.clone_from(k);
        }
        for (k, val) in self.networks.iter_mut() {
            val.name.clone_from(k);
        }
    }

    /// Walk through the canister_id.json at the top-level and each network
    /// to initialize the name -> string map
    #[tracing::instrument(skip(self))]
    fn init_canister_id_map(&mut self, path: &Path) -> Result<()> {
        self.process_canister_id_file(&path.parent().unwrap().join("canister_ids.json"))
            .ok();
        let network_keys = self
            .networks
            .keys()
            .map(|k| k.to_owned())
            .collect::<Vec<_>>();
        for key in network_keys {
            self.process_canister_id_file(
                &path
                    .parent()
                    .unwrap()
                    .join(".dfx")
                    .join(key)
                    .join("canister_ids.json"),
            )
            .ok();
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    fn process_canister_id_file(&mut self, path: &Path) -> Result<()> {
        type CanisterIdMap = HashMap<String, HashMap<String, String>>;

        let f = File::open(path)?;
        let id_map: CanisterIdMap = serde_json::from_reader(f)?;

        debug!("Reading canister IDs from {:?}", path);

        for (name, val) in id_map {
            if let Some(canister) = self.canisters.get_mut(&name) {
                for (network_name, id) in val {
                    canister
                        .network_id_map
                        .entry(network_name.clone())
                        .or_insert_with(|| {
                            debug!("Added canister id {} for network {}", &id, &network_name);
                            id
                        });
                }
            }
        }

        Ok(())
    }

    #[tracing::instrument]
    pub fn try_from_dscvr_for_network(
        root_file: DSCVRConfig,
        network: &str,
    ) -> std::result::Result<Self, Error> {
        let mut canisters = HashMap::new();
        let mut networks = HashMap::new();
        for (canister_name, canister) in root_file.canisters {
            let mut controllers = HashMap::new();
            let network_controllers: Vec<(&str, Option<&String>)> = canister
                .networks
                .iter()
                .map(|(network_name, network)| {
                    (network_name.as_str(), network.controllers.as_ref())
                })
                .collect();
            for (network_name, group_name) in network_controllers {
                if group_name.is_some() {
                    let cg = root_file
                        .controller_groups
                        .as_ref()
                        .ok_or_else(|| {
                            DfxGenerationError::ControllerGroupMissing(canister_name.to_string())
                        })?
                        .get(group_name.unwrap())
                        .ok_or_else(|| {
                            DfxGenerationError::ControllerGroupMismatch(
                                group_name.unwrap().clone(),
                                canister_name.to_string(),
                            )
                        })?;
                    controllers.insert(network_name.to_string(), cg.controllers.to_owned());
                }
            }

            for (network_name, provider, instances, wallet) in
                canister.networks.iter().map(|(name, cfg)| {
                    let instances = cfg.get_all_instances();
                    (name, &cfg.provider, instances, cfg.wallet.as_ref())
                })
            {
                // Only push the IC (production) canisters to dfx.json
                if network_name == network {
                    for instance in instances {
                        let mut wallets = HashMap::default();
                        if let Some(w) = wallet {
                            wallets.insert(network_name.clone(), w.clone());
                        }

                        canisters.insert(
                            instance.name.to_string(),
                            DfxCanister {
                                name: instance.name.to_string(),
                                candid: canister.candid.clone(),
                                wasm: canister.wasm.clone(),
                                build: canister.build.clone(),
                                supports_init_params: canister
                                    .supports_init_params
                                    .unwrap_or(false),
                                supports_stable_storage_backup_restore: canister
                                    .supports_stable_storage_backup_restore
                                    .unwrap_or(false),
                                network_id_map: Default::default(),
                                wallet: Some(wallets),
                                controllers: Some(controllers.clone()),
                            },
                        );
                    }
                }

                // Only insert non-local networks
                // dfx cli does not allow setting the local
                // network with a provider, so we won't write
                // it to file.
                if network_name != LOCAL_NETWORK_NAME {
                    networks.insert(
                        network_name.clone(),
                        DfxNetwork {
                            name: network_name.clone(),
                            providers: Some(vec![provider.clone()]),
                            bind: Some(provider.clone()),
                        },
                    );
                }
            }
        }

        Ok(Self {
            canisters,
            dfx: DFX_VERSION.to_string(),
            networks,
            canister_setup_order: vec![],
        })
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct CanisterIds {
    #[serde(flatten)]
    pub ids: HashMap<String, HashMap<String, String>>,
}

impl From<DSCVRConfig> for CanisterIds {
    fn from(root: DSCVRConfig) -> Self {
        let mut ids: HashMap<String, HashMap<String, String>> = HashMap::default();
        for canister_definition in root.canisters.into_values() {
            for (network_name, network_definition) in canister_definition.networks {
                if let Some(provisioned) = network_definition.provisioned_instances {
                    for can in provisioned {
                        if let Some(id) = can.id {
                            ids.entry(can.name)
                                .or_default()
                                .entry(network_name.clone())
                                .or_insert(id);
                        }
                    }
                }

                if let Some(available) = network_definition.available_instances {
                    for can in available {
                        if let Some(id) = can.id {
                            ids.entry(can.name)
                                .or_default()
                                .entry(network_name.clone())
                                .or_insert(id);
                        }
                    }
                }
            }
        }

        Self { ids }
    }
}
