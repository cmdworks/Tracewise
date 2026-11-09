//! Configuration, validation, secret detection, and trust checks for sharing.
use std::fs;
use std::io;
use std::path::{Component, Path};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ShareConfig {
    pub remote: String,
    pub level: String,
    /// `refs = "hidden"`: batches go to `refs/trail/device/<id>` (not shown as branches); default: `refs/heads/trail/device/<id>`.
    #[serde(default)]
    pub hidden_refs: bool,
    /// `encrypt = true`: batches are encrypted with the project key before they are pushed (S4).
    #[serde(default)]
    pub encrypt: bool,
}

pub fn validate_remote_url(url: &str) -> io::Result<()> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "remote URL cannot be empty"));
    }
    if trimmed.starts_with('-') {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "remote URL cannot start with '-' (preventing flag injection)"));
    }
    if trimmed.contains('\n') || trimmed.contains('\r') {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "remote URL cannot contain newline characters"));
    }

    let is_allowed = trimmed.starts_with('/')
        || trimmed.starts_with("file://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ssh://")
        || is_scp_style(trimmed);

    if !is_allowed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("remote URL '{}' is not an allowed scheme (must be https://, ssh://, git@host:, file://, or absolute path)", trimmed),
        ));
    }
    Ok(())
}

fn is_scp_style(s: &str) -> bool {
    if s.contains("://") {
        return false;
    }
    if let Some((user_host, path)) = s.split_once(':') {
        if path.is_empty() || path.starts_with(':') || user_host.is_empty() || user_host.contains('/') {
            return false;
        }
        if let Some((user, host)) = user_host.split_once('@') {
            return !user.is_empty()
                && !host.is_empty()
                && !user.starts_with('-')
                && !host.starts_with('-')
                && user.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
                && host.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.');
        }
    }
    false
}

pub fn read_trailconfig(root: &Path) -> Option<ShareConfig> {
    let content = fs::read_to_string(root.join(".trailconfig")).ok()?;
    let mut remote = None;
    let mut level = None;
    let mut hidden_refs = false;
    let mut encrypt = false;
    let mut in_share = false;
    for line in content.lines() {
        let mut in_quote = false;
        let mut comment_start = None;
        for (i, c) in line.char_indices() {
            if c == '"' {
                in_quote = !in_quote;
            } else if c == '#' && !in_quote {
                comment_start = Some(i);
                break;
            }
        }
        let effective = match comment_start {
            Some(idx) => &line[..idx],
            None => line,
        }
        .trim();

        if effective.is_empty() {
            continue;
        }

        if effective.starts_with('[') && effective.ends_with(']') {
            let section = effective[1..effective.len() - 1].trim();
            in_share = section == "share";
            continue;
        }

        if in_share {
            if let Some((k, v)) = effective.split_once('=') {
                let k = k.trim();
                let mut v = v.trim();
                if v.starts_with('"') && v.ends_with('"') && v.len() >= 2 {
                    v = &v[1..v.len() - 1];
                }
                if k == "remote" {
                    if validate_remote_url(v).is_ok() {
                        remote = Some(v.to_string());
                    }
                } else if k == "level" {
                    level = Some(v.to_string());
                } else if k == "refs" {
                    hidden_refs = v == "hidden";
                } else if k == "encrypt" {
                    encrypt = v == "true";
                }
            }
        }
    }
    match (remote, level) {
        (Some(r), Some(l)) => Some(ShareConfig { remote: r, level: l, hidden_refs, encrypt }),
        _ => None,
    }
}

/// `https://user:token@host/x.git` -> `https://user@host/x.git`: a password never goes into `.trailconfig` or the data
/// (C07); git asks its credential helper for it.
pub fn strip_credentials(url: &str) -> String {
    let Some((scheme, rest)) = url.split_once("://") else { return url.to_string() };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    match rest[..authority_end].rsplit_once('@') {
        Some((userinfo, host)) => {
            let user = userinfo.split(':').next().unwrap_or("");
            format!("{scheme}://{}{host}{}", if user.is_empty() { String::new() } else { format!("{user}@") }, &rest[authority_end..])
        }
        None => url.to_string(),
    }
}

pub fn write_trailconfig(root: &Path, remote: &str, level: &str) -> io::Result<()> {
    write_trailconfig_with(root, remote, level, false)
}

/// `hidden_refs` adds `refs = "hidden"`: the batches are kept under `refs/trail/device/`.
pub fn write_trailconfig_with(root: &Path, remote: &str, level: &str, hidden_refs: bool) -> io::Result<()> {
    write_trailconfig_full(root, remote, level, hidden_refs, false)
}

/// Like `write_trailconfig_with`, with `encrypt = true` when asked (S4).
pub fn write_trailconfig_full(root: &Path, remote: &str, level: &str, hidden_refs: bool, encrypt: bool) -> io::Result<()> {
    validate_remote_url(remote)?;
    let remote = &strip_credentials(remote);
    let refs = if hidden_refs { "refs = \"hidden\"\n" } else { "" };
    let enc = if encrypt { "encrypt = true\n" } else { "" };
    let block = format!("[share]\nremote = \"{}\"\nlevel = \"{}\"\n{refs}{enc}", remote, level);
    let path = root.join(".trailconfig");
    // every other section (`[profile] share = true`, `[reasons]`, ...) is kept; only `[share]` is replaced
    let old = fs::read_to_string(&path).unwrap_or_default();
    let mut kept: Vec<&str> = Vec::new();
    let mut in_share = false;
    for line in old.lines() {
        let t = line.trim();
        if t.starts_with('[') && t.ends_with(']') {
            in_share = t[1..t.len() - 1].trim() == "share";
        }
        if !in_share {
            kept.push(line);
        }
    }
    while kept.last().is_some_and(|l| l.trim().is_empty()) {
        kept.pop();
    }
    let mut text = kept.join("\n");
    if !text.is_empty() {
        text.push_str("\n\n");
    }
    fs::write(path, text + &block)
}

pub(crate) const TRUSTED_REMOTES_FILE: &str = "trusted_remotes";

pub fn is_remote_trusted(data_dir: &Path, remote: &str) -> bool {
    let remote = &strip_credentials(remote);
    fs::read_to_string(data_dir.join(TRUSTED_REMOTES_FILE)).map(|s| s.lines().any(|l| l.trim() == remote.trim())).unwrap_or(false)
}

pub fn trust_remote(data_dir: &Path, remote: &str) -> io::Result<()> {
    validate_remote_url(remote)?;
    let remote = &strip_credentials(remote);
    if is_remote_trusted(data_dir, remote) {
        return Ok(());
    }
    fs::create_dir_all(data_dir)?;
    let mut f = fs::OpenOptions::new().create(true).append(true).open(data_dir.join(TRUSTED_REMOTES_FILE))?;
    io::Write::write_all(&mut f, format!("{}\n", remote.trim()).as_bytes())
}

pub fn contains_secret(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    if text.lines().any(|l| l.contains("-----BEGIN ") && l.contains("PRIVATE KEY-----")) {
        return true;
    }
    const PREFIXES: [(&str, usize); 8] =
        [("AKIA", 16), ("ghp_", 30), ("gho_", 30), ("ghs_", 30), ("github_pat_", 30), ("xoxb-", 20), ("xoxp-", 20), ("sk_live_", 20)];
    PREFIXES.iter().any(|(prefix, min)| {
        text.match_indices(prefix).any(|(i, _)| {
            text[i + prefix.len()..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').count() >= *min
        })
    })
}

pub fn is_secret_path(path: &str) -> bool {
    let file_name = Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path);

    if file_name.starts_with(".env") {
        return true;
    }
    if file_name.starts_with("id_") {
        return true;
    }
    if file_name.ends_with(".pem") || file_name.ends_with(".key") || file_name.ends_with(".pfx") || file_name.ends_with(".p12") {
        return true;
    }
    false
}

pub fn is_safe_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 128 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

pub fn is_safe_rel_path(p: &str) -> bool {
    // 4096 and 255 bytes are the usual operating-system limits for a path and for one name in it
    if p.is_empty() || p.len() > 4096 || p.contains('\0') {
        return false;
    }
    let path = Path::new(p);
    !path.is_absolute()
        && path.components().all(|c| match c {
            Component::Normal(n) => n.to_str().is_some_and(|s| s.len() <= 255 && !s.eq_ignore_ascii_case(".git")),
            _ => false,
        })
}

pub(crate) fn owner_prefix(url: &str) -> Option<String> {
    let u = url.trim().trim_end_matches('/');
    let cut = u.rfind(['/', ':'])?;
    let prefix = &u[..=cut];
    let has_owner = if let Some((_, rest)) = prefix.split_once("://") {
        let mut parts = rest.split('/').filter(|x| !x.is_empty());
        let local_file = prefix.starts_with("file://");
        parts.next().is_some() && (local_file || parts.next().is_some())
    } else if let Some((_, rest)) = prefix.split_once(':') {
        rest.split('/').any(|x| !x.is_empty())
    } else {
        prefix.starts_with('/')
    };
    has_owner.then(|| prefix.to_ascii_lowercase())
}

/// The URL of the project's own `origin` remote, if it has one.
pub fn origin_url(repo_root: &Path) -> Option<String> {
    let out = super::batch::git_cmd().arg("-C").arg(repo_root).args(["config", "--get", "remote.origin.url"]).output().ok()?;
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !url.is_empty()).then_some(url)
}

pub fn may_sync_from(repo_root: &Path, data_dir: &Path, remote: &str) -> bool {
    if is_remote_trusted(data_dir, remote) {
        return true;
    }
    match (origin_url(repo_root).as_deref().and_then(owner_prefix), owner_prefix(remote)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
