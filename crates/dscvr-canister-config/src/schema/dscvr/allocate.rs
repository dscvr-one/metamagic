use super::*;
use crate::schema::dscvr::DSCVRGenerationError::NoAvailableCanisterInstances;

impl DSCVRConfig {
    /// Add `available_instances` to self for the
    /// specified canister and network.
    ///
    /// ### Inputs
    /// - `canister_name: &str` - Canister to add available_instances too
    /// - `network: &str` - Network on which to add the available_instances
    /// - `count: usize` - Number of available instances to add
    ///
    /// ### Returns
    /// - `Result<Vec<CanisterInstance, DSCVRGenerationError>` - Returns `Ok()` with a copy
    /// of the newly available canisters if successful.
    pub(crate) fn add_available_canisters(
        &mut self,
        canister_name: &str,
        network: &str,
        count: usize,
    ) -> std::result::Result<Vec<CanisterInstance>, Error> {
        let canister = self.get_canister_for_network_mut(canister_name, network)?;

        let mut next_canister = if let Some(provisioned) = &canister.provisioned_instances {
            if let Some(available) = &canister.available_instances {
                available.len() + provisioned.len() + 1
            } else {
                provisioned.len() + 1
            }
        } else if let Some(available) = &canister.available_instances {
            available.len() + 1
        } else {
            1
        };

        let total = next_canister + count;
        let mut new_canisters: Vec<CanisterInstance> = Vec::new();
        while next_canister < total {
            let name = format!("{}{NAME_DELIMITER}{}", canister_name, next_canister);
            new_canisters.push(CanisterInstance { name, id: None });
            next_canister += 1;
        }

        let mut available_instances = std::mem::take(
            canister
                .available_instances
                .as_mut()
                .unwrap_or(&mut Default::default()),
        );
        available_instances.append(&mut new_canisters.clone());

        canister.available_instances = Some(available_instances);

        Ok(new_canisters)
    }

    /// Update available canisters with IDs
    ///
    /// ### Inputs
    /// - `canister_name: &str` - Name of the canister with available instances
    /// - `network: &str` - Network the available canisters reside in
    /// - `canisters: Vec<CanisterInstance` - the canister objects to register
    ///
    /// ### Returns
    /// - `Result<(), DSCVRGenerationErr>` - Returns `Ok()` if able to update canister
    /// instances.
    pub(crate) fn register_available_canisters(
        &mut self,
        canister_name: &str,
        network: &str,
        canisters: Vec<CanisterInstance>,
    ) -> std::result::Result<(), Error> {
        let canister = self.get_canister_for_network_mut(canister_name, network)?;
        let available_instances = canister.available_instances.as_mut().ok_or_else(|| {
            NoAvailableCanisterInstances(
                canister_name.to_string(),
                network.to_string(),
                "Register".to_string(),
            )
        })?;
        for canister_instance in canisters {
            if let Some(instance) = available_instances
                .iter_mut()
                .find(|instance| instance.name == canister_instance.name)
            {
                instance.id = canister_instance.id;
            }
        }

        Ok(())
    }
}
