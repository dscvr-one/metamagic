use candid::pretty::candid::compile;
use candid_parser::check_file_with_imports;
use instrumented_error::Result;
use std::path::{Path, PathBuf};

/// Combines all imported candid files into a single file.
#[tracing::instrument]
pub fn combine_candid_files(path: &Path, output_file: &str) -> Result<Vec<PathBuf>> {
    let candid_path = Path::new(path);
    let result = check_file_with_imports(candid_path)?;
    // export the did to all defined networks
    let contents = compile(&result.0, &result.1);
    std::fs::write(output_file, contents)?;

    Ok(result.2)
}
