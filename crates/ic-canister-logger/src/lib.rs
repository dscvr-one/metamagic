//! Logging for canisters

use std::io::Write;

use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::time::FormatTime;

pub mod scoped_instruction_counter;

struct IcStdout;

impl Write for IcStdout {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        #[cfg(target_arch = "wasm32")]
        ic_cdk::print(std::str::from_utf8(buf).map_err(|_e| std::io::ErrorKind::InvalidData)?);
        #[cfg(not(target_arch = "wasm32"))]
        print!(
            "{}",
            std::str::from_utf8(buf).map_err(|_e| std::io::ErrorKind::InvalidData)?
        );

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct IcTimer;

#[cfg(not(target_arch = "wasm32"))]
fn current_time_nanos() -> u64 {
    time::OffsetDateTime::now_utc().unix_timestamp_nanos() as u64
}
#[cfg(target_arch = "wasm32")]
fn current_time_nanos() -> u64 {
    ic_cdk::api::time() as u64
}

impl FormatTime for IcTimer {
    fn format_time(&self, w: &mut Writer) -> std::fmt::Result {
        let now = current_time_nanos();
        w.write_str(&format!("{now}"))
    }
}

/// Init the logger for canisters
#[cfg(target_arch = "wasm32")]
pub fn init_logger() {
    use tracing::Level;
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    use tracing_subscriber::fmt::Layer;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Registry;

    let make_writer = || IcStdout;
    let log_layer = Layer::default()
        .with_writer(make_writer.with_max_level(Level::INFO))
        .with_timer(IcTimer);

    Registry::default().with(log_layer).init();
}
#[cfg(not(target_arch = "wasm32"))]
pub fn init_logger() {}
