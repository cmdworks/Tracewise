use super::*;

#[test]
fn test_daemon_plist_criteria() {
    let bin = Path::new("/Users/test/.tracewise/bin/tracewise");
    let log = Path::new("/Users/test/.tracewise/daemon.log");
    let xml = daemon_plist(bin, log);

    assert!(xml.contains("<key>KeepAlive</key><true/>"));
    assert!(xml.contains("<key>LegacyTimers</key><true/>"));
    assert!(xml.contains("<key>LowPriorityIO</key><true/>"));
    assert!(!xml.contains("<string>Background</string>"));
    assert!(xml.contains("<key>StandardErrorPath</key>"));
}

#[test]
fn test_sensor_plist_generation() {
    let bin = Path::new("/usr/local/libexec/tracewise/tracewise-sensor");
    let sock = Path::new("/var/run/tracewise.sock");
    let xml = sensor_plist(bin, sock);

    assert!(xml.contains("dev.tracewise.sensor"));
    assert!(xml.contains("--socket"));
}
