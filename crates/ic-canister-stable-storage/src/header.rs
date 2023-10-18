//! Header for stable storage
//!
use std::{
    io::{Read, Write},
    mem::size_of,
};

use candid::{CandidType, Deserialize};
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use serde::Serialize;

use super::data_format::DataFormatType;

/// Errors related to serializing and deserializing the header
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)] // self documenting
pub enum Error {
    #[error("Invalid content format {0}")]
    InvalidContentFormat(u64),
    #[error("IO error {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid header length {0} expecting {1}")]
    InvalidHeaderLength(u64, u64),
}

/// The header contains information that's critical to serializing the contents
/// and the footer
#[derive(Debug, CandidType, Serialize, Deserialize, Default, Clone, PartialEq, Eq)]
pub struct Header {
    /// Length of the header
    pub header_length: u64,
    /// Length of the content
    pub content_length: u64,
    /// Format of the content
    pub content_format: DataFormatType,
    /// Schema version of the content
    pub content_schema_version: u64,
    /// Number of instructions used for pre-upgrade
    pub pre_upgrade_instruction_count: u64,
}

// Index of the fields in the header struct
#[derive(PartialEq, PartialOrd, Eq, Ord)]
enum FieldIndex {
    ContentLength = 0,
    ContentFormat,
    ContentSchemaVersion,
    PreUpgradeInstructionCount,
    NumFields,
}

const U64_SIZE: usize = size_of::<u64>();

impl Header {
    /// Create a header with format and schema version
    pub fn new_from_format_and_schema(format: DataFormatType, schema_version: u64) -> Self {
        Self {
            header_length: FieldIndex::NumFields as u64,
            content_length: 0,
            content_format: format,
            content_schema_version: schema_version,
            pre_upgrade_instruction_count: 0,
        }
    }

    /// Create a header from a reader
    pub fn new_from_reader<R: Read>(reader: &mut R) -> std::result::Result<Self, Error> {
        let header_length = Self::read_u64(reader)?;
        if header_length > FieldIndex::NumFields as u64 {
            return Err(Error::InvalidHeaderLength(
                header_length,
                FieldIndex::NumFields as u64,
            ));
        }

        let fields = Self::read_n_u64(reader, header_length as usize)?;

        Self::new_from_vec(fields)
    }

    /// Create a header from an async reader
    pub async fn new_from_reader_async<R: AsyncRead + AsyncReadExt + Unpin>(
        reader: &mut R,
    ) -> std::result::Result<Self, Error> {
        let header_length = Self::read_u64_async(reader).await?;
        if header_length > FieldIndex::NumFields as u64 {
            return Err(Error::InvalidHeaderLength(
                header_length,
                FieldIndex::NumFields as u64,
            ));
        }

        let fields = Self::read_n_u64_async(reader, header_length as usize).await?;

        Self::new_from_vec(fields)
    }

    /// Create a header from a vector of u64
    fn new_from_vec(fields: Vec<u64>) -> std::result::Result<Self, Error> {
        let content_format = fields[FieldIndex::ContentFormat as usize].into();
        if content_format == DataFormatType::Unknown {
            return Err(Error::InvalidContentFormat(fields[3]));
        }

        Ok(Self {
            header_length: fields.len() as u64,
            content_length: fields[FieldIndex::ContentLength as usize],
            content_format,
            content_schema_version: fields[FieldIndex::ContentSchemaVersion as usize],
            pre_upgrade_instruction_count: fields[FieldIndex::PreUpgradeInstructionCount as usize],
        })
    }

    /// Write the header
    pub fn write<W: Write>(&self, writer: &mut W) -> std::result::Result<(), Error> {
        Ok(writer.write_all(&self.as_bytes())?)
    }

    /// Write the header async
    pub async fn write_async<W: AsyncWrite + AsyncWriteExt + Unpin>(
        &self,
        writer: &mut W,
    ) -> std::result::Result<(), Error> {
        Ok(writer.write_all(&self.as_bytes()).await?)
    }

    /// Return the number of bytes needed by all fields of the header
    pub fn num_all_fields_bytes(&self) -> u64 {
        // NumFields + 1 to include the header length
        (FieldIndex::NumFields as u64 + 1) * U64_SIZE as u64
    }

    /// Return the number of bytes needed by used by both the header and content
    pub fn num_content_and_header_bytes(&self) -> u64 {
        self.header_length * U64_SIZE as u64 + U64_SIZE as u64 + self.content_length
    }

    // Helper to read a single u64 from a reader
    fn read_u64<R: Read>(reader: &mut R) -> std::io::Result<u64> {
        let mut bytes = [0_u8; U64_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    // Helper to read a single u64 from a reader
    async fn read_u64_async<R: AsyncRead + AsyncReadExt + Unpin>(
        reader: &mut R,
    ) -> std::io::Result<u64> {
        let mut bytes = [0_u8; U64_SIZE];
        reader.read_exact(&mut bytes).await?;
        Ok(u64::from_le_bytes(bytes))
    }

    // Helper to read the next n u64s from a reader
    fn read_n_u64<R: Read>(reader: &mut R, count: usize) -> std::io::Result<Vec<u64>> {
        let mut bytes = vec![0_u8; count * U64_SIZE];
        reader.read_exact(&mut bytes)?;
        Ok(Self::bytes_to_u64(&bytes, count))
    }

    // Helper to read the next n u64s from a reader
    async fn read_n_u64_async<R: AsyncRead + AsyncReadExt + Unpin>(
        reader: &mut R,
        count: usize,
    ) -> std::io::Result<Vec<u64>> {
        let mut bytes = vec![0_u8; count * U64_SIZE];
        reader.read_exact(&mut bytes).await?;
        Ok(Self::bytes_to_u64(&bytes, count))
    }

    fn bytes_to_u64(bytes: &[u8], count: usize) -> Vec<u64> {
        (0..count)
            .map(|i| {
                u64::from_le_bytes(bytes[i * U64_SIZE..(i + 1) * U64_SIZE].try_into().unwrap())
            })
            .collect::<Vec<_>>()
    }

    /// Return the header as bytes
    pub fn as_bytes(&self) -> Vec<u8> {
        let vals = [
            FieldIndex::NumFields as u64,
            self.content_length,
            self.content_format as u64,
            self.content_schema_version,
            self.pre_upgrade_instruction_count,
        ];
        vals.into_iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let header = Header {
            header_length: FieldIndex::NumFields as u64,
            content_length: 100,
            content_format: DataFormatType::MsgPack,
            content_schema_version: 10,
            pre_upgrade_instruction_count: 100,
        };

        let mut bytes = vec![];
        header.write(&mut bytes).unwrap();

        assert_eq!(bytes.len(), U64_SIZE * 5);

        let roundtrip_header = Header::new_from_reader(&mut bytes.as_slice()).unwrap();
        assert_eq!(header, roundtrip_header);

        assert_eq!(
            header.num_content_and_header_bytes(),
            bytes.len() as u64 + header.content_length,
        );
    }

    #[tokio::test]
    async fn test_roundtrip_async() {
        let header = Header {
            header_length: FieldIndex::NumFields as u64,
            content_length: 100,
            content_format: DataFormatType::MsgPack,
            content_schema_version: 10,
            pre_upgrade_instruction_count: 100,
        };

        let mut bytes = vec![];
        header.write_async(&mut bytes).await.unwrap();

        assert_eq!(bytes.len(), U64_SIZE * 5);

        let roundtrip_header = Header::new_from_reader_async(&mut bytes.as_slice())
            .await
            .unwrap();
        assert_eq!(header, roundtrip_header);

        assert_eq!(
            header.num_content_and_header_bytes(),
            bytes.len() as u64 + header.content_length,
        );
    }
}
