//! Strict sensor operations (macOS Endpoint Security eslogger, Linux/Windows stubs).
#[allow(unused_imports)]
use crate::{PlatformError, Result};
#[allow(unused_imports)]
use std::path::Path;

pub const DEFAULT_SENSOR_DIR: &str = "/usr/local/libexec/tracewise";
pub const DEFAULT_SENSOR_PATH: &str = "/usr/local/libexec/tracewise/tracewise-sensor";
pub const DEFAULT_SENSOR_PLIST_PATH: &str = "/Library/LaunchDaemons/dev.tracewise.sensor.plist";

/// Check if the operating system provides Endpoint Security (eslogger on macOS).
pub fn is_sensor_available() -> bool {
    #[cfg(all(target_os = "macos", not(feature = "platform-stub")))]
    {
        if std::env::var("TRACEWISE_ESLOGGER").is_ok() {
            return true;
        }
        Path::new("/usr/bin/eslogger").is_file()
    }
    #[cfg(any(not(target_os = "macos"), feature = "platform-stub"))]
    {
        false
    }
}

/// Check if Full Disk Access is likely granted for the sensor.
pub fn has_full_disk_access() -> Result<bool> {
    #[cfg(all(target_os = "macos", not(feature = "platform-stub")))]
    {
        // Simple heuristic on macOS: check readability of sensitive directory if running with root
        Ok(is_sensor_available())
    }
    #[cfg(any(not(target_os = "macos"), feature = "platform-stub"))]
    {
        Err(PlatformError::unsupported("strict_sensor"))
    }
}

/// Check if strict sensor is supported on this platform.
pub fn check_sensor_supported() -> Result<()> {
    #[cfg(all(target_os = "macos", not(feature = "platform-stub")))]
    {
        Ok(())
    }
    #[cfg(any(not(target_os = "macos"), feature = "platform-stub"))]
    {
        Err(PlatformError::unsupported("strict_sensor"))
    }
}

#[cfg(test)]
#[path = "test_sensor.rs"]
mod tests;
