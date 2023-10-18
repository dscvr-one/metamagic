//! Generates clients that are complementary to those provided
//! by didc (https://github.com/dfinity/candid/tree/master/tools/didc)

pub mod rust_canister_agent;
pub mod util;

#[cfg(test)]
mod test {
    use super::*;
    use std::path::Path;
    const DID: &str = "../../canisters/society_rs/society-common.did";
    #[test]
    #[ignore]
    fn test_generate() {
        let output_dir: std::path::PathBuf = Path::new("src").join("gen");
        std::fs::create_dir_all("src/gen").unwrap();
        let _ = rust_canister_agent::generate(
            Path::new(DID),
            &output_dir.join("dscvr_tx_log_agent.rs"),
        )
        .expect("Something good to happen");
    }
}
