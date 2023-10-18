//! Helper methods to manage identity

use std::str::FromStr;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use ic_agent::{
    identity::{BasicIdentity, Secp256k1Identity},
    Identity,
};
use instrumented_error::Result;
use ring::signature::Ed25519KeyPair;
use serde::{Deserialize, Serialize};

/// Wrapper to implement our own deserialize method to initialize
/// an identity from a pem file path
#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct IdentityFromFile(PathBuf);

impl FromStr for IdentityFromFile {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(IdentityFromFile(PathBuf::from(s)))
    }
}

impl IdentityFromFile {
    /// Return the inner Identity
    #[tracing::instrument]
    pub fn identity(&self) -> Result<Arc<dyn Identity>> {
        create_identity_from_pem(&self.0)
    }

    /// Join the parent path to the inner path.
    /// This is needed since the identity may have been initialized without the parent
    /// path during deserialization.
    pub fn join_parent(&mut self, parent: &Path) {
        self.0 = parent.join(&self.0);
    }

    /// Return the path
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Create an identity from a pem file
#[tracing::instrument()]
pub fn create_identity_from_pem(pem_file: &Path) -> Result<Arc<dyn Identity>> {
    if let Ok(id) = BasicIdentity::from_pem_file(pem_file) {
        Ok(Arc::new(id))
    } else {
        Ok(Arc::new(Secp256k1Identity::from_pem_file(pem_file)?))
    }
}

impl<'de> Deserialize<'de> for IdentityFromFile {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(IdentityFromFile(
            Path::new(&String::deserialize(deserializer)?).to_path_buf(),
        ))
    }
}

/// Create a temporary identity that exists for the lifetime of a program
#[tracing::instrument]
pub fn new_ephemeral_identity() -> Result<Arc<dyn Identity>> {
    let rng = ring::rand::SystemRandom::new();
    let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng)?.as_ref().to_vec();
    let keypair = Ed25519KeyPair::from_pkcs8(&pkcs8_bytes)?;
    Ok(Arc::from(BasicIdentity::from_key_pair(keypair)))
}
