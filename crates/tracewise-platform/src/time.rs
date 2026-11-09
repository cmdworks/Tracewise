//! Local time representation without external heavy timezone databases.

/// Format timestamp seconds as "HH:MM" in the machine's local time.
pub fn local_hhmm(timestamp_secs: i64) -> String {
    #[cfg(unix)]
    {
        let secs = timestamp_secs as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe { libc::localtime_r(&secs, &mut tm) };
        format!("{:02}:{:02}", tm.tm_hour, tm.tm_min)
    }
    #[cfg(not(unix))]
    {
        let hours = ((timestamp_secs / 3600) % 24).abs();
        let mins = ((timestamp_secs / 60) % 60).abs();
        format!("{:02}:{:02}", hours, mins)
    }
}

#[cfg(test)]
#[path = "test_time.rs"]
mod tests;
