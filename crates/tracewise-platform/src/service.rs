//! Background service management (launchd on macOS, systemd on Linux stubs).
use crate::{PlatformError, Result};
use std::path::{Path, PathBuf};
#[allow(unused_imports)]
use std::process::Command;

pub const AGENT_LABEL: &str = "dev.tracewise.daemon";
pub const SENSOR_LABEL: &str = "dev.tracewise.sensor";
pub const LAUNCHCTL: &str = "launchctl";

/// Return the command to bootstrap a system service.
pub fn system_bootstrap_command(plist_path: &str) -> Vec<String> {
    vec!["sudo".to_string(), LAUNCHCTL.to_string(), "bootstrap".to_string(), "system".to_string(), plist_path.to_string()]
}

/// Return the command to bootout a system service.
pub fn system_bootout_command(label: &str) -> Vec<String> {
    vec!["sudo".to_string(), LAUNCHCTL.to_string(), "bootout".to_string(), format!("system/{label}")]
}

/// Return the launchctl domain for user services (e.g. `gui/<uid>` on macOS).
pub fn launchctl_domain() -> String {
    #[cfg(target_os = "macos")]
    {
        let uid = unsafe { libc::getuid() };
        format!("gui/{uid}")
    }
    #[cfg(not(target_os = "macos"))]
    {
        "gui/1000".to_string()
    }
}

/// Check if service management is supported on this platform.
pub fn check_service_management_supported() -> Result<()> {
    #[cfg(all(target_os = "macos", not(feature = "platform-stub")))]
    {
        Ok(())
    }
    #[cfg(any(not(target_os = "macos"), feature = "platform-stub"))]
    {
        Err(PlatformError::unsupported("service_management"))
    }
}

/// Generate LaunchAgent plist for the Tracewise daemon.
/// Criterion P09: KeepAlive true, LegacyTimers true, LowPriorityIO true,
/// never ProcessType Background, and valid log paths.
pub fn daemon_plist(bin_path: &Path, log_path: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{AGENT_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>daemon</string>
        <string>run</string>
    </array>
    <key>KeepAlive</key><true/>
    <key>LowPriorityIO</key><true/>
    <key>LegacyTimers</key><true/>
    <key>StandardOutPath</key><string>{}</string>
    <key>StandardErrorPath</key><string>{}</string>
</dict>
</plist>
"#,
        bin_path.display(),
        log_path.display(),
        log_path.display()
    )
}

/// Generate LaunchDaemon plist for the Tracewise strict sensor.
pub fn sensor_plist(sensor_bin: &Path, socket_path: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>{SENSOR_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--socket</string>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>ProcessType</key><string>Background</string>
    <key>StandardErrorPath</key><string>/var/log/tracewise-sensor.log</string>
</dict>
</plist>
"#,
        sensor_bin.display(),
        socket_path.display()
    )
}

/// The plist file of the job launchd has loaded under `label`, if any.
pub fn loaded_plist_path(label: &str) -> Option<PathBuf> {
    #[cfg(all(target_os = "macos", not(feature = "platform-stub")))]
    {
        let out = Command::new("launchctl").args(["print", &format!("{}/{label}", launchctl_domain())]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).lines().find_map(|l| l.trim().strip_prefix("path = ").map(PathBuf::from))
    }
    #[cfg(any(not(target_os = "macos"), feature = "platform-stub"))]
    {
        let _ = label;
        None
    }
}

/// True when the loaded job comes from `expected_plist`.
pub fn loaded_job_is_ours(label: &str, expected_plist: &Path) -> bool {
    loaded_plist_path(label).is_some_and(|p| p == expected_plist)
}

/// Load (bootstrap) a background service with launchd.
/// Criterion P10: If a job is loaded under the same label from another file, leaves it alone.
pub fn load_service(plist_path: &Path, label: &str) -> Result<()> {
    #[cfg(all(target_os = "macos", not(feature = "platform-stub")))]
    {
        match loaded_plist_path(label) {
            Some(p) if p != plist_path => {
                // foreign job; leave it alone
                return Ok(());
            }
            Some(_) => {
                let _ = unload_service(label, Some(plist_path));
            }
            None => {}
        }
        let out = Command::new("launchctl")
            .args(["bootstrap", &launchctl_domain(), &plist_path.to_string_lossy()])
            .output()
            .map_err(|e| PlatformError::Io(e.to_string()))?;
        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(PlatformError::Io(format!("launchctl bootstrap failed: {err}")))
        }
    }
    #[cfg(any(not(target_os = "macos"), feature = "platform-stub"))]
    {
        let _ = (plist_path, label);
        Err(PlatformError::unsupported("background_service"))
    }
}

/// Unload (bootout) a background service with launchd.
pub fn unload_service(label: &str, expected_plist: Option<&Path>) -> Result<()> {
    #[cfg(all(target_os = "macos", not(feature = "platform-stub")))]
    {
        if let Some(expected) = expected_plist {
            if !loaded_job_is_ours(label, expected) {
                // Not ours or not loaded; do not touch
                return Ok(());
            }
        }
        let out = Command::new("launchctl")
            .args(["bootout", &format!("{}/{label}", launchctl_domain())])
            .output()
            .map_err(|e| PlatformError::Io(e.to_string()))?;
        if out.status.success() {
            Ok(())
        } else {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(PlatformError::Io(format!("launchctl bootout failed: {err}")))
        }
    }
    #[cfg(any(not(target_os = "macos"), feature = "platform-stub"))]
    {
        let _ = (label, expected_plist);
        Err(PlatformError::unsupported("background_service"))
    }
}

#[cfg(test)]
#[path = "test_service.rs"]
mod tests;
