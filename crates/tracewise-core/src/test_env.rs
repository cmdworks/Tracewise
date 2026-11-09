//! Shared by tests that set process environment variables: they must not run at the same time, and must not run as
//! if an agent had started them.
use std::sync::Mutex;

pub static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn clear_agent_markers() {
    std::env::remove_var("CLAUDECODE");
    std::env::remove_var("ANTIGRAVITY_AGENT");
    std::env::remove_var("ANTIGRAVITY_CONVERSATION_ID");
}
