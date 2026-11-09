use crate::cli::{Commands, ShimsAction};
use crate::shim_detect::{is_tracewise_bin_name, SHIM_TOOLS};
use std::fs;
use std::path::Path;
use std::process;

fn is_tracewise_shim_symlink(p: &Path, self_exe: Option<&Path>) -> bool {
    if !p.is_symlink() {
        return false;
    }
    let Ok(target) = fs::read_link(p) else {
        return false;
    };
    if let Some(self_p) = self_exe {
        if p.canonicalize().ok().as_deref() == Some(self_p) {
            return true;
        }
    }
    target.file_name().and_then(|n| n.to_str()).map(is_tracewise_bin_name).unwrap_or(false)
}

pub fn install_shims(dir: &Path, self_exe: &Path) -> Result<Vec<String>, String> {
    fs::create_dir_all(dir).map_err(|e| format!("failed to create dir {}: {e}", dir.display()))?;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !SHIM_TOOLS.contains(&name_str.as_ref()) && is_tracewise_shim_symlink(&p, Some(self_exe)) {
                let _ = fs::remove_file(&p);
            }
        }
    }

    let mut installed_tools = Vec::new();
    for &tool in SHIM_TOOLS {
        let link_path = dir.join(tool);
        tracewise_platform::shims::create_shim_link(self_exe, &link_path)
            .map_err(|e| format!("failed to create shim link {} -> {}: {e}", link_path.display(), self_exe.display()))?;
        installed_tools.push(tool.to_string());
    }

    Ok(installed_tools)
}

pub fn uninstall_shims(dir: &Path, self_exe: Option<&Path>) -> usize {
    let mut removed = 0;
    if dir.is_dir() {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if is_tracewise_shim_symlink(&p, self_exe) && fs::remove_file(&p).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    removed
}

pub(crate) fn run(json: bool, command: Commands) {
    match command {
        Commands::Shims { action } => match action {
            ShimsAction::Install { dir } => {
                let self_exe = match std::env::current_exe() {
                    Ok(p) => match p.canonicalize() {
                        Ok(c) => c,
                        Err(_) => p,
                    },
                    Err(e) => {
                        eprintln!("tracewise: cannot determine current executable path: {e}");
                        process::exit(1);
                    }
                };

                let target_bin =
                    self_exe.parent().map(|p| p.join("tracewise-shim")).filter(|p| p.is_file()).unwrap_or_else(|| self_exe.clone());

                match install_shims(&dir, &target_bin) {
                    Ok(tools) => {
                        if json {
                            println!(
                                "{}",
                                serde_json::json!({
                                    "dir": dir.to_string_lossy(),
                                    "tools": tools
                                })
                            );
                        } else {
                            println!("Installed shims in {}", dir.display());
                            println!("Add to PATH: export PATH=\"{}:$PATH\"", dir.display());
                        }
                    }
                    Err(e) => {
                        eprintln!("tracewise: {e}");
                        process::exit(1);
                    }
                }
            }
            ShimsAction::Uninstall { dir } => {
                let self_exe = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok());

                let removed = uninstall_shims(&dir, self_exe.as_deref());

                if json {
                    println!("{}", serde_json::json!({ "removed": removed }));
                } else {
                    println!("Removed {} shim(s) from {}", removed, dir.display());
                }
            }
        },
        _ => unreachable!("dispatched to the wrong group"),
    }
}

#[cfg(test)]
#[path = "test_shim_install.rs"]
mod tests;
