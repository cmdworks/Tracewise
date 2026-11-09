//! Operating-system abstraction layer for Tracewise.
//! All operating-system specific code (sysctl, proc_pidinfo, launchctl, signals,
//! local sockets, path normalization, shell snippets, shim links) lives in this crate.

pub mod paths;
pub mod private;
pub mod process;
pub mod sensor;
pub mod service;
pub mod shell;
pub mod shims;
pub mod signal;
pub mod socket;
pub mod time;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PlatformError {
    Unsupported { feature: &'static str, reason: Option<String> },
    Io(String),
    NotFound(String),
}

impl std::fmt::Display for PlatformError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported { feature, reason } => {
                if let Some(r) = reason {
                    write!(f, "{feature}: not supported on this platform ({r})")
                } else {
                    write!(f, "{feature}: not supported on this platform")
                }
            }
            Self::Io(msg) => write!(f, "{msg}"),
            Self::NotFound(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PlatformError {}

impl From<std::io::Error> for PlatformError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, PlatformError>;

impl PlatformError {
    pub fn unsupported(feature: &'static str) -> Self {
        Self::Unsupported { feature, reason: None }
    }

    pub fn unsupported_reason(feature: &'static str, reason: impl Into<String>) -> Self {
        Self::Unsupported { feature, reason: Some(reason.into()) }
    }
}

#[cfg(test)]
#[path = "test_lib.rs"]
mod tests;
