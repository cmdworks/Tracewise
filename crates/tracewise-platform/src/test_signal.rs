use super::*;

#[test]
fn test_is_process_running_current() {
    let me = std::process::id() as i32;
    assert!(is_process_running(me));
    assert!(!is_process_running(99999999));
}

#[test]
fn test_stale_pid_never_signalled() {
    // PID 1 is launchd/init (not a tracewise daemon)
    // wake_daemon and terminate_daemon must refuse to signal it and return Ok(false)
    let res_wake = wake_daemon(1).unwrap();
    assert!(!res_wake, "stale/unrelated PID must not be signalled by wake_daemon");

    let res_term = terminate_daemon(1).unwrap();
    assert!(!res_term, "stale/unrelated PID must not be signalled by terminate_daemon");
}
