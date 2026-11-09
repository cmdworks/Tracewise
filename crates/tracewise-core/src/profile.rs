//! Profile configuration: core (default), coordination, policy, strict, share.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub core: bool,
    pub coordination: bool,
    pub policy: bool,
    pub strict: bool,
    pub share: bool,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self::core()
    }
}

impl ProfileConfig {
    pub fn core() -> Self {
        Self { core: true, coordination: false, policy: false, strict: false, share: false }
    }

    pub fn full() -> Self {
        Self { core: true, coordination: true, policy: true, strict: true, share: true }
    }

    pub fn is_full(&self) -> bool {
        self.coordination && self.policy && self.strict && self.share
    }

    pub fn is_core(&self) -> bool {
        !self.coordination && !self.policy && !self.strict && !self.share
    }

    /// Read profile from a repository root, taking environment variables into account.
    pub fn from_root(root: &Path) -> Self {
        if let Ok(env) = std::env::var("TRAIL_PROFILE") {
            let lower = env.trim().to_lowercase();
            if lower == "full" {
                return Self::full();
            } else if lower == "core" {
                return Self::core();
            }
        }

        let mut config = if std::env::var_os("TRAIL_LOCK_FLAGS").is_some() { Self::full() } else { Self::core() };
        if let Ok(global_cfg) = std::fs::read_to_string(crate::get_trail_home().join("config.toml")) {
            Self::parse_trailconfig_content(&global_cfg, &mut config);
        }
        let config_file = root.join(".trailconfig");
        let content = match std::fs::read_to_string(&config_file) {
            Ok(c) => c,
            Err(_) => return config,
        };

        Self::parse_trailconfig_content(&content, &mut config);
        config
    }

    /// Read profile from the current working directory or nearest repository root.
    pub fn from_cwd() -> Self {
        if let Ok(env) = std::env::var("TRAIL_PROFILE") {
            let lower = env.trim().to_lowercase();
            if lower == "full" {
                return Self::full();
            } else if lower == "core" {
                return Self::core();
            }
        }

        if let Ok(cwd) = std::env::current_dir() {
            let mut curr: Option<PathBuf> = Some(cwd);
            while let Some(dir) = curr {
                if dir.join(".trailconfig").is_file() || dir.join(".git").exists() {
                    return Self::from_root(&dir);
                }
                curr = dir.parent().map(|p| p.to_path_buf());
            }
        }

        let mut config = if std::env::var_os("TRAIL_LOCK_FLAGS").is_some() { Self::full() } else { Self::core() };
        if let Ok(global_cfg) = std::fs::read_to_string(crate::get_trail_home().join("config.toml")) {
            Self::parse_trailconfig_content(&global_cfg, &mut config);
        }
        config
    }

    /// Parse `[profile]` section from `.trailconfig` content.
    pub fn parse_trailconfig_content(content: &str, config: &mut Self) {
        let mut in_profile_section = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let sec = trimmed[1..trimmed.len() - 1].trim();
                in_profile_section = sec == "profile";
                if in_profile_section {
                    *config = Self::core();
                }
                continue;
            }
            if !in_profile_section {
                continue;
            }

            if let Some((key, val)) = trimmed.split_once('=') {
                let k = key.trim().to_lowercase();
                let v = val.split('#').next().unwrap_or("").trim().trim_matches('"').to_lowercase();

                match k.as_str() {
                    "name" | "profile" => {
                        if v == "full" {
                            *config = Self::full();
                        } else if v == "core" {
                            *config = Self::core();
                        }
                    }
                    "coordination" => {
                        config.coordination = v == "true" || v == "1" || v == "yes";
                    }
                    "policy" => {
                        config.policy = v == "true" || v == "1" || v == "yes";
                    }
                    "strict" => {
                        config.strict = v == "true" || v == "1" || v == "yes";
                    }
                    "share" => {
                        config.share = v == "true" || v == "1" || v == "yes";
                    }
                    _ => {}
                }
            }
        }
    }

    /// Mistakes in the `[profile]` section that are otherwise ignored without a word (`name = "coordination"` stays core).
    pub fn config_warnings(content: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_profile = false;
        for line in content.lines().map(str::trim) {
            if line.starts_with('[') && line.ends_with(']') {
                in_profile = line[1..line.len() - 1].trim() == "profile";
            } else if in_profile {
                let Some((k, v)) = line.split_once('=') else { continue };
                let (k, v) = (k.trim().to_lowercase(), v.split('#').next().unwrap_or("").trim().trim_matches('"').to_lowercase());
                match k.as_str() {
                    "name" | "profile" if v != "core" && v != "full" => out.push(format!(
                        "[profile] name = \"{v}\" is not core or full, so it is ignored; to switch one feature on write `{v} = true`"
                    )),
                    "name" | "profile" | "coordination" | "policy" | "strict" | "share" => {}
                    other => out.push(format!("[profile] has an unknown key `{other}`, ignored")),
                }
            }
        }
        out
    }

    /// Summary description of active features, e.g. "core", "core + coordination", or "full".
    pub fn description(&self) -> String {
        if self.is_full() {
            "full".to_string()
        } else if self.is_core() {
            "core".to_string()
        } else {
            let mut parts = vec!["core"];
            if self.coordination {
                parts.push("coordination");
            }
            if self.policy {
                parts.push("policy");
            }
            if self.strict {
                parts.push("strict");
            }
            if self.share {
                parts.push("share");
            }
            parts.join(" + ")
        }
    }
}

#[cfg(test)]
#[path = "test_profile.rs"]
mod tests;

#[cfg(test)]
mod warning_tests {
    use super::ProfileConfig;

    #[test]
    fn a_wrong_profile_name_or_key_is_reported() {
        let w = ProfileConfig::config_warnings("[profile]\nname = \"coordination\"\npolcy = true\n[reasons]\nmode = \"warn\"\n");
        assert_eq!(w.len(), 2);
        assert!(w[0].contains("coordination = true") && w[1].contains("polcy"));
        assert!(ProfileConfig::config_warnings("[profile]\nname = \"core\"\ncoordination = true\n").is_empty());
    }
}
