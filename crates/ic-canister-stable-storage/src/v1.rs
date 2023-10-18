//! v1 implementation of stable storage layout

use dscvr_interface::Interface;
use std::io::{Read, Seek, Write};
use tracing::info;

use super::movable_io::{MovableReader, MovableWriter};
use crate::data_format::DataFormatType;
use crate::data_format::{MsgPackAdapter, SerdeDataFormat};
use crate::header::Header;
use crate::transient::Transient;
use crate::Error;

/// Serialize using v1 layout
#[tracing::instrument(skip(t, writer, system))]
pub fn save<T, W: Write + Seek>(system: &dyn Interface, writer: &mut W, t: &T) -> Result<(), Error>
where
    MsgPackAdapter: SerdeDataFormat,
    T: serde::Serialize,
{
    info!("Starting save");
    MsgPackAdapter::serialize(MovableWriter::new(writer), t)?;
    info!("Total Pre Upgrade Instruction Count {}", system.instruction_counter());
    Ok(())
}

/// Deserialize using v1 layout
#[tracing::instrument(skip(reader, system))]
pub fn restore<R: Read + Seek, T>(system: &dyn Interface, reader: &mut R) -> Result<(Header, Transient, T), Error>
where
    MsgPackAdapter: SerdeDataFormat,
    T: for<'a> serde::Deserialize<'a>,
{
    let t: T = MsgPackAdapter::deserialize(MovableReader::new(reader))?;
    let header = Header {
        content_length: reader.stream_position()?,
        content_format: DataFormatType::MsgPack,
        ..Default::default()
    };
    let count = system.instruction_counter();
    let transient = Transient {
        post_upgrade_instruction_count: count,
        ..Default::default()
    };
    info!("Post Upgrade Instruction Count {}", count);
    Ok((header, transient, t))
}
