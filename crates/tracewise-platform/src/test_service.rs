use super::*;

#[test]
fn test_daemon_plist_criteria() {
    let bin = Path::new("/Users/test/.trail/bin/trail");
    let log = Path::new("/Users/test/.trail/daemon.log");
    let xml = daemon_plist(bin, log);

    assert!(xml.contains("<key>KeepAlive</key><true/>"));
    assert!(xml.contains("<key>LegacyTimers</key><true/>"));
    assert!(xml.contains("<key>LowPriorityIO</key><true/>"));
    assert!(!xml.contains("<string>Background</string>"));
    assert!(xml.contains("<key>StandardErrorPath</key>"));
}

#[test]
fn test_sensor_plist_generation() {
    let bin = Path::new("/usr/local/libexec/trail/trail-sensor");
    let sock = Path::new("/var/run/trail.sock");
    let xml = sensor_plist(bin, sock);

    assert!(xml.contains("dev.trail.sensor"));
    assert!(xml.contains("--socket"));
}
