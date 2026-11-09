//! The list of repositories one daemon watches: `$TRAIL_HOME/repos`, one `work tree<TAB>data folder` per line.
//! `trail init` and `trail daemon start` add to it; a repository whose folders are gone drops out when read.

use std::fs;
use std::path::{Path, PathBuf};

pub struct Repo {
    pub work_tree: PathBuf,
    pub data_dir: PathBuf,
}

fn file_in(home: &Path) -> PathBuf {
    home.join("repos")
}

/// The registered repositories that still have a Tracewise database.
pub fn list() -> Vec<Repo> {
    list_in(&crate::get_trail_home())
}

pub fn list_in(home: &Path) -> Vec<Repo> {
    let mut seen = std::collections::HashSet::new();
    fs::read_to_string(file_in(home))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let (wt, dd) = l.split_once('\t')?;
            let (wt, dd) = (PathBuf::from(wt), PathBuf::from(dd));
            (dd.join("trail.db").is_file() && wt.is_dir() && seen.insert(wt.clone())).then_some(Repo { work_tree: wt, data_dir: dd })
        })
        .collect()
}

/// Adds a repository; true when it was not listed yet.
pub fn register(work_tree: &Path, data_dir: &Path) -> bool {
    register_in(&crate::get_trail_home(), work_tree, data_dir)
}

pub fn register_in(home: &Path, work_tree: &Path, data_dir: &Path) -> bool {
    if list_in(home).iter().any(|r| r.work_tree == work_tree) {
        prune_in(home);
        return false;
    }
    let path = file_in(home);
    if let Some(dir) = path.parent() {
        let _ = tracewise_platform::private::create_dir_all(dir);
    }
    // rewrite the surviving entries too, so the file never keeps repositories that are gone
    let mut text: String = list_in(home).iter().map(|r| format!("{}\t{}\n", r.work_tree.display(), r.data_dir.display())).collect();
    text.push_str(&format!("{}\t{}\n", work_tree.display(), data_dir.display()));
    let tmp = path.with_extension("tmp");
    tracewise_platform::private::write(&tmp, text).is_ok() && fs::rename(&tmp, &path).is_ok()
}

/// Rewrites the list without the repositories that are gone (J01); nothing is written when it is already clean.
pub fn prune_in(home: &Path) {
    let path = file_in(home);
    let text: String = list_in(home).iter().map(|r| format!("{}\t{}\n", r.work_tree.display(), r.data_dir.display())).collect();
    if fs::read_to_string(&path).is_ok_and(|old| old != text) {
        let tmp = path.with_extension("tmp");
        if tracewise_platform::private::write(&tmp, text).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

/// Tells a running daemon to read the list again (SIGHUP). Never signals a stale pid.
pub fn wake_daemon() {
    let pid = fs::read_to_string(crate::get_trail_home().join("daemon.pid")).ok().and_then(|t| t.trim().parse::<i32>().ok());
    if let Some(pid) = pid.filter(|p| *p > 1) {
        let _ = tracewise_platform::signal::wake_daemon(pid);
    }
}

#[cfg(test)]
#[path = "test_repos.rs"]
mod tests;
