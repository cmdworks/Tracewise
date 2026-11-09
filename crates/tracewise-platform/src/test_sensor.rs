use super::*;

#[test]
fn test_sensor_paths_defined() {
    assert!(DEFAULT_SENSOR_DIR.contains("libexec"));
    assert!(DEFAULT_SENSOR_PATH.contains("trail-sensor"));
    assert!(DEFAULT_SENSOR_PLIST_PATH.contains("LaunchDaemons"));
}
