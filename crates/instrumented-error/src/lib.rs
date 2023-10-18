//! A boxed error type to enforce error instrumentation
//
// This uses the tracing crate to generate errors that are easy to read,
// have file and line info as well as sufficient context to debug
// the error.

use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use tracing_error::InstrumentError;
use tracing_error::TracedError;

/// A boxed error that's instrumented via tracing
pub struct BoxedInstrumentedError(Box<dyn std::error::Error + 'static + Send + Sync>);

impl BoxedInstrumentedError {
    /// Return the inner boxed error
    pub fn into_std_error(self) -> BoxedInstrumentedStdError {
        BoxedInstrumentedStdError(self.0)
    }
}

impl Debug for BoxedInstrumentedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)?;
        if let Some(source) = self.0.source() {
            return Debug::fmt(&source, f);
        }
        Ok(())
    }
}

impl Display for BoxedInstrumentedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)?;
        if let Some(source) = self.0.source() {
            return Display::fmt(&source, f);
        }
        Ok(())
    }
}

impl<E> From<E> for BoxedInstrumentedError
where
    E: InstrumentError<Instrumented = TracedError<E>>
        + std::error::Error
        + 'static
        + Send
        + Sync
        + Sized,
{
    #[inline]
    fn from(val: E) -> Self {
        BoxedInstrumentedError(Box::new(val.in_current_span()))
    }
}

/// Error alias for a boxed instrumented error.
pub type Error = BoxedInstrumentedError;
/// Result alias for a boxed instrumented error.
pub type Result<T> = std::result::Result<T, Error>;

/// Helper trait to convert non-error types (such as string) to our instrument error type
/// This is needed since the blanket implementation above prevents us from providing
/// specialized implementations for non-error types.
pub trait IntoInstrumentedError {
    /// Convert to our error type
    fn into_instrumented_error(self) -> Error;
}

/// Helper trait to convert result with non-error types (such as string) to our instrument error type
/// This is needed since the blanket implementation above prevents us from providing
/// specialized implementations for non-error types.
pub trait IntoInstrumentedResult<T> {
    /// Convert our result type
    fn into_instrumented_result(self) -> Result<T>;
}

impl IntoInstrumentedError for String {
    fn into_instrumented_error(self) -> Error {
        use std::fmt;

        // This is the same implementation as Box<dyn Error> in the rust library
        struct StringError(String);

        impl std::error::Error for StringError {
            #[allow(deprecated)]
            #[inline]
            fn description(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for StringError {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        // Purposefully skip printing "StringError(..)"
        impl fmt::Debug for StringError {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }

        StringError(self).into()
    }
}

impl<T, E> IntoInstrumentedResult<T> for std::result::Result<T, E>
where
    E: IntoInstrumentedError,
{
    #[inline]
    fn into_instrumented_result(self) -> Result<T> {
        self.map_err(|e| e.into_instrumented_error())
    }
}

/// StdError implementation. Ideally, we would be able implement Error on
/// `BoxedInstrumentedError` directly. However, the blanket From<E> implementation
/// for `BoxedInstrumentedError` prevents us from doing this.
#[derive(Debug)]
pub struct BoxedInstrumentedStdError(Box<dyn std::error::Error + 'static + Send + Sync>);

impl std::error::Error for BoxedInstrumentedStdError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl std::fmt::Display for BoxedInstrumentedStdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}
