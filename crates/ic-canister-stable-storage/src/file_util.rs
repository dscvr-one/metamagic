use dscvr_interface::edge::Edge;
/// Utilities for restore/saving to v2 version of the stable storage format
use instrumented_error::Result;
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, BufWriter, Write},
};

use crate::transient::Transient;
use crate::v2::{restore, save};
use crate::{header::Header, migration};

/// Save state to a file
#[tracing::instrument(skip(t, header, transient))]
pub fn save_to_file<T>(file: &str, t: &T, header: Header, transient: &Transient) -> Result<()>
where
    T: serde::Serialize,
{
    let mut writer = BufWriter::new(OpenOptions::new().write(true).create(true).truncate(true).open(file)?);
    migration::set_stored_schema_version(header.content_schema_version);
    save(&Edge::default(), &mut writer, t, header, transient)?;
    writer.flush()?;
    Ok(())
}

/// Restore state from a file
#[tracing::instrument]
pub fn restore_from_file<T>(file: &str) -> Result<(Header, Transient, T)>
where
    for<'a> T: serde::Deserialize<'a>,
{
    let mut reader = BufReader::new(File::open(file)?);
    Ok(restore(&Edge::default(), &mut reader)?)
}
