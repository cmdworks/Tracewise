//! Companion repository sharing, push, fetch, and rebuild for Tracewise.

pub(crate) mod batch;
pub(crate) mod config;
pub mod crypt;
pub(crate) mod fetch;
pub(crate) mod push;
pub mod sign;
#[cfg(test)]
mod tests;
mod warnings;

pub use batch::{ensure_companion_repo, BatchMeta};
pub use config::{
    contains_secret, is_remote_trusted, is_safe_id, is_safe_rel_path, is_secret_path, may_sync_from, origin_url, read_trailconfig,
    strip_credentials, trust_remote, validate_remote_url, write_trailconfig, write_trailconfig_full, write_trailconfig_with, ShareConfig,
};
pub use fetch::{fetch, import_batch_dir, rebuild};
pub use push::{delete_own_ref, push};
pub use warnings::sharing_warnings;
