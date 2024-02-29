use candid::parser::typing::{check_file_with_options, CheckFileOptions};
use instrumented_error::Result;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Combines all imported candid files into a single file.
#[tracing::instrument]
pub fn combine_candid_files(path: &Path, output_file: &str) -> Result<BTreeSet<PathBuf>> {
    let candid_path = Path::new(path);
    let result = check_file_with_options(
        candid_path,
        &CheckFileOptions {
            pretty_errors: false,
            combine_actors: true,
        },
    )?;
    // export the did to all defined networks
    let contents = candid::bindings::candid::compile(&result.types, &result.actor);
    std::fs::write(output_file, contents)?;

    Ok(result.imports)
}
