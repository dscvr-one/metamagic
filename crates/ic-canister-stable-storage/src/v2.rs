//! v2 implementation of stable storage layout

use dscvr_interface::Interface;
use std::io::SeekFrom;
use std::io::{Read, Seek, Write};
use tracing::info;
use tracing::warn;

use super::data_format::{BincodeAdapter, MsgPackAdapter, SerdeDataFormat};
use super::header::Header;
use super::movable_io::{MovableReader, MovableWriter};
use super::transient::Transient;
use super::Error;
use crate::data_format::DataFormatType;
use crate::header;
use crate::migration::set_stored_schema_version;

/// Serialize using v2 layout
#[tracing::instrument(skip_all)]
pub fn save<T, W: Write + Seek>(
    interface: &dyn Interface,
    writer: &mut W,
    t: &T,
    mut header: Header,
    transient: &Transient,
) -> Result<(), Error>
where
    T: serde::Serialize,
{
    info!("started inst_count={}", interface.instruction_counter());

    if transient.skip_next_save {
        info!("Skipping next save");
    } else {
        info!("Starting save");

        // write the contents first
        let header_len = header.num_all_fields_bytes();
        let start_pos = writer.stream_position()?;

        writer.seek(SeekFrom::Start(start_pos + header_len))?;

        info!("Content start {}", start_pos + header_len);

        match header.content_format {
            DataFormatType::MsgPack => {
                MsgPackAdapter::serialize(MovableWriter::new(writer), t)?;
            }
            DataFormatType::Bincode => {
                BincodeAdapter::serialize(MovableWriter::new(writer), t)?;
            }
            _ => {
                return Err(header::Error::InvalidContentFormat(header.content_format as u64).into());
            }
        }

        let content_end_pos = writer.stream_position()?;
        // update content length
        header.content_length = content_end_pos - start_pos - header_len;
        // update instruction count
        header.pre_upgrade_instruction_count = interface.instruction_counter();

        // save header
        writer.seek(SeekFrom::Start(start_pos))?;
        header.write(writer)?;

        info!(
            "finished inst_count={} memory_usage={}",
            interface.instruction_counter(),
            interface.get_memory_usage()
        );
    }
    Ok(())
}

/// Deserialize from stable storage using v2 layout
#[tracing::instrument(skip_all)]
pub fn restore<R: Read + Seek, T>(interface: &dyn Interface, reader: &mut R) -> Result<(Header, Transient, T), Error>
where
    T: for<'a> serde::Deserialize<'a>,
{
    info!("started inst_count={}", interface.instruction_counter());

    let header = Header::new_from_reader(reader)?;
    info!("read header schema_version={}", header.content_schema_version);
    set_stored_schema_version(header.content_schema_version);
    let content_start_pos = reader.stream_position()?;

    info!("Content start {}", content_start_pos);

    let t: T = match header.content_format {
        DataFormatType::MsgPack => MsgPackAdapter::deserialize(MovableReader::new(reader))?,
        DataFormatType::Bincode => BincodeAdapter::deserialize(MovableReader::new(reader))?,
        _ => {
            return Err(header::Error::InvalidContentFormat(header.content_format as u64).into());
        }
    };
    let content_end_pos = reader.stream_position()?;
    let content_length = content_end_pos - content_start_pos;

    if content_length != header.content_length {
        warn!(
            "Unexpected content length expected: {}, actual: {}",
            header.content_length, content_length
        );
    }

    let count = interface.instruction_counter();
    let transient = Transient {
        post_upgrade_instruction_count: count,
        ..Default::default()
    };
    info!(
        "finished inst_count={} memory_usage={}",
        interface.instruction_counter(),
        interface.get_memory_usage()
    );
    Ok((header, transient, t))
}
