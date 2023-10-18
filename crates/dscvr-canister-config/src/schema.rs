pub mod dfx;
pub mod dscvr;

use crate::prelude::*;
use crate::schema::dfx::CanisterIds;
use crate::schema::dscvr::CanisterInstance;
use dfx::DfxConfig;
use dscvr::DSCVRConfig;
use instrumented_error::IntoInstrumentedResult;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter};
use std::path::Path;

const DEFAULT_DFX_CONFIG_PATH: &str = "./dfx.json";
const DEFAULT_DSCVR_CONFIG_PATH: &str = "./dscvr.json";
const DEFAULT_CANISTER_IDS_PATH: &str = "./canister_ids.json";
const LOCAL_DSCVR_CONFIG_PATH: &str = "./dscvr.local.json";
const LOCAL_CANISTER_IDS_PATH: &str = "./.dfx/local/canister_ids.json";
const LOCAL_NETWORK_NAME: &str = "local";
const PRODUCTION_NETWORK_NAME: &str = "ic";

fn get_config<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_reader::<_, T>(BufReader::new(std::fs::File::open(path).expect("File exists")))
        .map_err(|err| format!("{err}"))
        .into_instrumented_result()
}

fn write_config<T>(path: &str, config: &T) -> Result<()>
where
    T: Serialize,
{
    serde_json::to_writer(
        BufWriter::new(std::fs::File::create(path).expect("File created")),
        config,
    )
    .map_err(|err| format!("{err}"))
    .into_instrumented_result()
}

fn generate_dfx_json(dscvr_cfg: DSCVRConfig, network: &str) -> Result<DfxConfig> {
    DfxConfig::try_from_dscvr_for_network(dscvr_cfg, network)
        .map_err(|err| format!("{err}"))
        .into_instrumented_result()
}

fn generate_canister_ids_json(dscvr_cfg: DSCVRConfig) -> Result<CanisterIds> {
    Ok(dscvr_cfg.into())
}

/// Attempts to generate `dfx.json` and `canister_ids.json` for the dfx cli
/// using the `ic` network from `dscvr.json`
///
/// This will also create a new `dscvr.local.json` if it doesn't already exist.
pub fn generate_config_from_production() -> Result<DSCVRConfig> {
    let mut dscvr_cfg = DSCVRConfig::try_new(PRODUCTION_NETWORK_NAME)?;
    DSCVRConfig::merge_local(&mut dscvr_cfg)?;
    Ok(dscvr_cfg)
}

pub fn generate_dfx_config_for_network(dscvr_cfg: &DSCVRConfig, network: &str) -> Result<()> {
    write_config(DEFAULT_DFX_CONFIG_PATH, &generate_dfx_json(dscvr_cfg.clone(), network)?)?;
    write_config(
        DEFAULT_CANISTER_IDS_PATH,
        &generate_canister_ids_json(dscvr_cfg.clone())?,
    )
}

/// Marks new instances of a canister as available in the `dscvr.json` file,
/// and copies those new canister names into `dfx.json`.  The dfx cli can
/// then create new instances and assign ids.
///
/// ### Inputs
/// - `canister: &str` - Canister level name to allocate for
/// - `network: &str` - Network to allocate instances of `canister` in
/// - `count: usize` - Number of new instances to make available
///
/// ### Returns
/// - `Result<Vec<CanisterInstance>>` - returns `Ok()` with a copy of the
/// newly available canister instances on success.
pub fn allocate_canisters(canister: &str, network: &str, count: usize) -> Result<Vec<CanisterInstance>> {
    let mut dscvr_cfg = DSCVRConfig::try_new(network)?;
    let available_canisters = dscvr_cfg
        .add_available_canisters(canister, network, count)
        .map_err(|err| format!("{err}"))
        .into_instrumented_result()?;
    dscvr_cfg.write_config(network)?;
    write_config(DEFAULT_DFX_CONFIG_PATH, &generate_dfx_json(dscvr_cfg, network)?)?;
    Ok(available_canisters)
}

/// Reads `canister_ids.json` for the specified `canister, network` pair
/// and updates all instance ids in `dscvr.json` to match.
///
/// This should be called after `dfx canister create` has been issued on
/// `CanisterInstances` returned by `allocate_canisters`
///
/// ### Inputs
/// - `canister: &str` - Canister level name to augment IDs for
/// - `network: &str` - Network to augment instances of `canister` in
///
/// ### Returns
/// - `Result<())>` - returns `Ok()` if the augmentation was a success
pub fn augment_canister_ids(canister: &str, network: &str) -> Result<()> {
    let canister_id_path = if network == LOCAL_NETWORK_NAME {
        Path::new(LOCAL_CANISTER_IDS_PATH)
    } else {
        Path::new(DEFAULT_CANISTER_IDS_PATH)
    };

    let mut dscvr_cfg = DSCVRConfig::try_new(network)?;
    let canister_ids = get_config::<CanisterIds>(canister_id_path)?;
    let canisters: Vec<CanisterInstance> = canister_ids
        .ids
        .into_iter()
        .map(|(name, canister_map)| {
            let id = canister_map.into_iter().find_map(
                |(network_name, id)| {
                    if network_name == *network {
                        Some(id)
                    } else {
                        None
                    }
                },
            );
            CanisterInstance { name, id }
        })
        .collect();
    dscvr_cfg
        .register_available_canisters(canister, network, canisters)
        .map_err(|err| format!("{err}"))
        .into_instrumented_result()?;
    dscvr_cfg.write_config(network)?;
    Ok(())
}

/// Gets a set of available_instances to provision for a specific
/// canister and network.  Instances will be updated for the specified
/// network and canister.
///
/// ### Inputs
/// - `config: &mut DSCVRConfig` - Config object to modify
/// - `canister: &str` - Canister to provision for
/// - `network: &str` - Network to provision for
/// - `count: usize` - Number of available instances to provision
///
/// ### Returns
/// - `Result<Vec<CanisterInstance>>` - Returns `Ok()` with a `Vec<CanisterInstance>`
/// that were provisioned.  These can be used to perform canister operations.

pub fn provision_canisters(
    config: &mut DSCVRConfig,
    canister: &str,
    network: &str,
    count: usize,
) -> Result<Vec<CanisterInstance>> {
    config
        .provision_canisters(canister, network, count)
        .map_err(|err| format!("{err}"))
        .into_instrumented_result()
}

/// Commit a config object to file for a specific network.
/// Use after a successful provisioning.
pub fn commit_config(config: &DSCVRConfig, network: &str) -> Result<()> {
    config.write_config(network)?;
    Ok(())
}
