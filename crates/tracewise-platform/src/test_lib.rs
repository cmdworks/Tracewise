use super::*;

#[test]
fn unsupported_names_feature_process() {
    let err = PlatformError::unsupported("process_inspection");
    let msg = err.to_string();
    assert!(msg.contains("process_inspection"), "error message must contain feature name: {msg}");
}

#[test]
fn unsupported_names_feature_service() {
    let err = PlatformError::unsupported("background_service");
    let msg = err.to_string();
    assert!(msg.contains("background_service"), "error message must contain feature name: {msg}");
}

#[test]
fn unsupported_names_feature_flags() {
    let err = PlatformError::unsupported("file_flags");
    let msg = err.to_string();
    assert!(msg.contains("file_flags"), "error message must contain feature name: {msg}");
}

#[test]
fn unsupported_names_feature_strict_sensor() {
    let err = PlatformError::unsupported_reason("strict_sensor", "no root access");
    let msg = err.to_string();
    assert!(msg.contains("strict_sensor"), "error message must contain feature name: {msg}");
    assert!(msg.contains("no root access"), "error message must contain reason: {msg}");
}
