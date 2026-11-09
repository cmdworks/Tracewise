//! Path normalization and Tracewise home directory discovery.
use std::path::{Path, PathBuf};

/// Normalize virtual path aliases (e.g. `/private/var` vs `/var` on macOS).
pub fn normalize_virtual_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("/private/var/") {
        PathBuf::from(format!("/var/{rest}"))
    } else if s == "/private/var" {
        PathBuf::from("/var")
    } else if let Some(rest) = s.strip_prefix("/private/tmp/") {
        PathBuf::from(format!("/tmp/{rest}"))
    } else if s == "/private/tmp" {
        PathBuf::from("/tmp")
    } else if let Some(rest) = s.strip_prefix("/private/etc/") {
        PathBuf::from(format!("/etc/{rest}"))
    } else if s == "/private/etc" {
        PathBuf::from("/etc")
    } else {
        path.to_path_buf()
    }
}

/// Normalize Windows path formats: convert `\` to `/` and strip extended-length prefix (`\\?\`).
pub fn normalize_windows_separators(path: &str) -> String {
    let stripped = path.strip_prefix(r"\\?\").unwrap_or(path);
    stripped.replace('\\', "/")
}

/// Compare two paths for equality considering platform-specific aliases.
pub fn paths_equal(a: &Path, b: &Path) -> bool {
    let norm_a = normalize_virtual_path(a);
    let norm_b = normalize_virtual_path(b);
    let str_a = normalize_windows_separators(&norm_a.to_string_lossy());
    let str_b = normalize_windows_separators(&norm_b.to_string_lossy());
    str_a == str_b
}

/// Locate Tracewise home directory: `TRACEWISE_HOME` env var, else `~/.tracewise`.
pub fn tracewise_home_dir() -> PathBuf {
    std::env::var_os("TRACEWISE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".tracewise")))
        .unwrap_or_else(|| PathBuf::from(".tracewise"))
}

/// Return both /private/... and non-/private/... spellings of a macOS virtual path.
pub fn path_spellings(p: &str) -> Vec<String> {
    let mut v = vec![p.to_string()];
    if let Some(rest) = p.strip_prefix("/private").filter(|r| r.starts_with('/')) {
        v.push(rest.to_string());
    } else if ["/var/", "/tmp/", "/etc/"].iter().any(|d| p.starts_with(d)) {
        v.push(format!("/private{p}"));
    }
    v
}

/// Compute `path` relative to `work_tree`, taking into account virtual aliases (e.g. `/private/var` vs `/var`).
pub fn relative_to(path: &Path, work_tree: &Path) -> Option<PathBuf> {
    let norm_path = normalize_virtual_path(path);
    let norm_wt = normalize_virtual_path(work_tree);
    if let Ok(rel) = norm_path.strip_prefix(&norm_wt) {
        return Some(rel.to_path_buf());
    }
    if let (Ok(p_canon), Ok(wt_canon)) = (path.canonicalize(), work_tree.canonicalize()) {
        let np = normalize_virtual_path(&p_canon);
        let nwt = normalize_virtual_path(&wt_canon);
        if let Ok(rel) = np.strip_prefix(&nwt) {
            return Some(rel.to_path_buf());
        }
    }
    None
}

/// Files and folders the operating system writes on its own (Finder's `.DS_Store`, AppleDouble `._*` files, `Thumbs.db`,
/// `desktop.ini`, Spotlight and trash folders). They are noise, not work: Tracewise never records them and does not wake for them.
pub fn is_os_junk(path: &Path) -> bool {
    const FILES: [&str; 4] = [".DS_Store", "Thumbs.db", "desktop.ini", ".localized"];
    const DIRS: [&str; 5] = [".Spotlight-V100", ".Trashes", ".fseventsd", ".TemporaryItems", ".DocumentRevisions-V100"];
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    FILES.contains(&name)
        || (name.starts_with("._") && name.len() > 2)
        || path.components().any(|c| c.as_os_str().to_str().is_some_and(|c| DIRS.contains(&c)))
}

/// Normalise a path string relative to `work_tree`.
pub fn normalize_path(path_str: &str, work_tree: &Path) -> String {
    let p = Path::new(path_str);
    if let Some(rel) = relative_to(p, work_tree) {
        return rel.to_string_lossy().to_string();
    }
    path_str.trim_start_matches("./").to_string()
}

#[cfg(test)]
#[path = "test_paths.rs"]
mod tests;
