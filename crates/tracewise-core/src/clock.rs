//! The clock Tracewise stamps records with. `TRACEWISE_NOW=<RFC 3339 time>` freezes it at that time so tests can move it back an hour,
//! skew two devices or go to the year 2038. An agent cannot use it: under an agent marker the real time is read.
use chrono::{DateTime, Utc};
use std::sync::OnceLock;

static FIXED: OnceLock<Option<DateTime<Utc>>> = OnceLock::new();

/// The current time, or the time in `TRACEWISE_NOW` when it is set and no agent is running this process.
pub fn now() -> DateTime<Utc> {
    let fixed = FIXED.get_or_init(|| {
        let value = std::env::var("TRACEWISE_NOW").ok()?;
        let by_agent = crate::model::Actor::from_env().is_some_and(|a| a.kind == "agent");
        DateTime::parse_from_rfc3339(value.trim()).ok().map(|t| t.with_timezone(&Utc)).filter(|_| !by_agent)
    });
    fixed.unwrap_or_else(Utc::now)
}

/// `now()` as the millisecond RFC 3339 text records carry.
pub fn now_text() -> String {
    now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

#[cfg(test)]
#[path = "test_clock.rs"]
mod tests;
