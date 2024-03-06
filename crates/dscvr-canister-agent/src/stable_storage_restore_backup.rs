use std::time::Duration;

use super::*;
use async_stream::try_stream;
use candid::Encode;
use futures::TryStreamExt;
use futures::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, SinkExt};
use ic_canister_stable_storage::{data_format::DataFormatType, header::Header, transient::Transient};
use instrumented_error::{BoxedInstrumentedError, Result};
use serde_bytes::{ByteBuf, Bytes};
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
use tracing::debug;

const BACKUP_CHUNK_SIZE: u64 = 1024 * 1024 * 5 / 2;
const RESTORE_CHUNK_SIZE: u64 = 2096000;

#[derive(Clone, Debug, CandidType, Deserialize, Serialize)]
pub struct CanisterStats {
    pub now: u64,
    pub memory_usage: u64,
    pub cycles: u64,
    pub stable_storage_usage_bytes: u64,
    pub last_upgraded: u64,
    pub version: String,
}

#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)] // self documenting
pub enum ErrorKind {
    #[error("Backup length mismatch expected {0} actual {1}")]
    BackupLengthMismatch(usize, usize),
    #[error("Canister stable storage not initialized")]
    CanisterStableStorageNotInitialized,
}

impl CanisterAgent {
    /// Get the stable storage header
    #[tracing::instrument(skip(self))]
    pub async fn get_stable_storage_info(&self) -> Result<(Header, Transient)> {
        let bytes = Encode!()?;
        Ok(Decode!(
            self.query("stable_storage_info", bytes).await?.as_slice(),
            Header,
            Transient
        )?)
    }

    #[tracing::instrument(skip(self))]
    async fn backup_stable_storage_chunk(&self, offset: u64, len: u64) -> Result<Vec<u8>> {
        if len <= offset {
            return Ok(vec![]);
        }

        debug!("Fetching {} of {}", offset, len);

        let bytes = Encode!(&offset, &std::cmp::min(BACKUP_CHUNK_SIZE, len - offset))?;
        Ok(Decode!(self.query("backup_stable_storage", bytes).await?.as_slice(), ByteBuf)?.into_vec())
    }

    /// Backup the stable storage of a canister to a writer
    #[tracing::instrument(skip_all)]
    pub async fn backup_stable_storage<W>(&self, mut writer: W) -> Result<()>
    where
        W: AsyncWriteExt + AsyncWrite + Unpin,
    {
        let (header, _) = self.get_stable_storage_info().await?;

        if header.content_format == DataFormatType::Unknown {
            return Err(ErrorKind::CanisterStableStorageNotInitialized.into());
        }

        let len = header.num_all_fields_bytes() + header.content_length;
        let count = len / BACKUP_CHUNK_SIZE + 1;
        let mut total_written = 0;
        stream::iter(0..count)
            .map(|idx| {
                let offset = idx * BACKUP_CHUNK_SIZE;
                self.backup_stable_storage_chunk(offset, len)
            })
            .buffered(10)
            .map(|item| {
                if let Ok(item) = item.as_ref() {
                    total_written += item.len();
                }
                item
            })
            .forward((&mut writer).into_sink().sink_err_into::<BoxedInstrumentedError>())
            .await?;
        let len = len as usize;
        if total_written != len {
            return Err(ErrorKind::BackupLengthMismatch(len, total_written)
                .in_current_span()
                .into());
        }
        writer.flush().await?;
        Ok(())
    }

    /// Restore the stable storage of a canister from a reader
    #[tracing::instrument(skip_all)]
    pub async fn restore_stable_storage<R>(&self, mut reader: R, restore_offest: Option<u64>) -> Result<()>
    where
        R: AsyncReadExt + AsyncRead + Unpin + Send + 'static,
    {
        let header = Header::new_from_reader_async(&mut reader).await?;
        let len = header.num_content_and_header_bytes();

        // grow the stable storage to at least be the total size we need
        {
            let bytes = candid::Encode!(&len)?;
            self.update("init_stable_storage", bytes).await?;
        }

        let header_bytes = header.as_bytes();
        let header_bytes_len = header_bytes.len() as u64;
        let restore_offset = restore_offest.unwrap_or(header_bytes_len);

        // restore the header
        debug!("Restoring header");
        {
            let bytes = candid::Encode!(&(0_u64), &Bytes::new(&header_bytes))?;
            self.update("restore_stable_storage", bytes).await?;
        }

        let stream = try_stream! {
            for offset in (restore_offset..len).step_by(RESTORE_CHUNK_SIZE as usize) {
                let size = std::cmp::min(
                    RESTORE_CHUNK_SIZE,
                    header.content_length - (offset - header_bytes_len),
                );
                let mut buf = vec![0u8; size as usize];
                reader.read_exact(&mut buf).await?;
                yield (buf, offset);
            }
        };

        let retry_strategy = ExponentialBackoff::from_millis(2000)
            .max_delay(Duration::from_secs(10))
            .map(jitter) // add jitter to delays
            .take(5);

        stream
            .map_ok(|(buf, offset)| {
                let buf = Arc::new(buf);
                Retry::spawn(retry_strategy.clone(), move || {
                    self.clone().restore(buf.clone(), len, offset)
                })
            })
            .try_buffer_unordered(10)
            .try_for_each(|_| async { Ok(()) })
            .await?;

        {
            let bytes = candid::Encode!(&true)?;
            self.update("set_restore_from_stable_storage", bytes).await?;
        }

        Ok(())
    }

    async fn restore(self: CanisterAgent, bytes: Arc<Vec<u8>>, len: u64, offset: u64) -> Result<()> {
        debug!("Restoring {} of {}", offset, len);

        let ret = {
            let encoded = candid::Encode!(&offset, &Bytes::new(&bytes[..]))?;
            self.update("restore_stable_storage", encoded).await.map(|_| ())
        };

        if let Err(e) = ret.as_ref() {
            debug!("Failed restoring {} of {} {:?}", offset, len, e);
        } else {
            debug!("Finished restoring {} of {}", offset, len);
        }

        ret
    }

    /// Return the default file name to be used for stable storage backups
    /// Note: This makes a network call to retrieve the module hash of the canister.
    pub async fn get_default_stable_storage_backup_file_name(&self, prefix: &str) -> Result<String> {
        let stats = self.canister_stats::<CanisterStats>().await?;
        let hash = hex::encode(self.canister_module_hash().await?);
        let time = OffsetDateTime::from_unix_timestamp_nanos(stats.last_upgraded as i128)?;
        Ok(format!(
            "{}_{}_{}",
            prefix,
            &hash[0..5],
            time.format(format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]"))
                .unwrap()
        ))
    }
}
