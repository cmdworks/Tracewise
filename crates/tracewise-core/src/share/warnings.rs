//! What `tracewise doctor` says about sharing and encryption. Warnings, never failures.
use super::{crypt, origin_url, read_tracewiseconfig};
use std::path::Path;

/// Plain-language warnings for this repository's sharing setup (empty when sharing is not set up or nothing is wrong).
pub fn sharing_warnings(work_tree: &Path, data_dir: &Path) -> Vec<String> {
    let Some(cfg) = read_tracewiseconfig(work_tree) else { return Vec::new() };
    let mut out = Vec::new();
    if cfg.encrypt {
        match crypt::load(data_dir).current {
            None => out.push(
                "encryption is on but this clone has no key: pushes are refused and shared batches cannot be read; import the project's key (`tracewise share key import`)"
                    .to_string(),
            ),
            Some(id) if !crypt::was_exported(data_dir, &id) => out.push(format!(
                "encryption is on and key {id} was never exported: if this clone is lost, nobody can read the shared history; run `tracewise share key export` and keep the result in a password manager"
            )),
            Some(_) => {}
        }
    } else if cfg.level == "full" && origin_url(work_tree).is_some_and(|o| o == cfg.remote) {
        out.push(
            "sharing at level full on the project's own remote: everyone who can read the repository can read file contents and reasons; use level summary or turn encryption on (`encrypt = true`)"
                .to_string(),
        );
    }
    out
}
