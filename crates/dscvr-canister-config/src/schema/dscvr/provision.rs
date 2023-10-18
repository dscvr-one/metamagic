use super::*;
use crate::schema::dscvr::DSCVRGenerationError::{NoAvailableCanisterInstances, ProvisionError};

impl DSCVRConfig {
    /// Moves a set of available instances to provisioned in the
    /// config file.  This is done to signal that a canister has
    /// had its wasm installed, is no longer able to be provisioned.
    ///
    /// ### Inputs
    /// - `canister_name: &str` - Name of the canister to provision
    /// instances for.
    /// - `network: &str` - The network to provision instances in.
    /// - `count: usize` - The number of instances to provision. If
    /// this is greater than the number of availble instances for the
    /// specified `canister & network`, will throw an error.
    ///
    /// ### Returns
    /// - `Result<Vec<CanisterInstance>, DSCVRGenerationError>` - returns
    /// `Ok()` with the `provisioned_instances` if successful.  These are
    /// the instances that should be passed to the `dfx canister install`
    /// command.
    pub(crate) fn provision_canisters(
        &mut self,
        canister_name: &str,
        network: &str,
        count: usize,
    ) -> std::result::Result<Vec<CanisterInstance>, Error> {
        let canister = self.get_canister_for_network_mut(canister_name, network)?;
        let mut instance_to_provision = vec![];
        let available_instances = canister.available_instances.as_mut().ok_or_else(|| {
            NoAvailableCanisterInstances(
                canister_name.to_string(),
                network.to_string(),
                "Provision".to_string(),
            )
        })?;
        while instance_to_provision.len() < count {
            instance_to_provision.push(available_instances.pop().ok_or_else(|| {
                ProvisionError("Not enough available canisters to provision".to_string())
            })?);
        }

        let instances_provisioned = instance_to_provision.clone();
        canister
            .provisioned_instances
            .as_mut()
            .unwrap_or(&mut Default::default())
            .append(&mut instance_to_provision);

        Ok(instances_provisioned)
    }
}
