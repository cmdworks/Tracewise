//! Launchd login item management delegated to `trail-platform::service`.
use super::*;

pub(crate) const AGENT_LABEL: &str = tracewise_platform::service::AGENT_LABEL;

pub(crate) fn agent_plist_text(env: &Env) -> String {
    tracewise_platform::service::daemon_plist(&env.bin().join("trail"), &env.root.join("daemon.log"))
}

pub(crate) fn login_item_installed(env: &Env) -> bool {
    env.agent_plist().is_file()
}

/// Loads (or reloads) the login item: launchd starts the daemon now and at every login.
pub(crate) fn login_item_load(env: &Env) {
    let _ = tracewise_platform::service::load_service(&env.agent_plist(), AGENT_LABEL);
}

/// Unloads it (launchd stops the daemon and does not restart it); the file stays for the next login.
pub(crate) fn login_item_unload(env: &Env) {
    let _ = tracewise_platform::service::unload_service(AGENT_LABEL, Some(&env.agent_plist()));
}

pub(crate) fn login_item_write(env: &Env) -> Result<(), String> {
    let plist = env.agent_plist();
    if let Some(dir) = plist.parent() {
        let _ = fs::create_dir_all(dir);
    }
    fs::write(&plist, agent_plist_text(env)).map_err(|e| format!("cannot write the login item: {e}"))
}

/// Stops a daemon that was started by hand (its pid file), so launchd's can take over.
pub(crate) fn stop_manual_daemon(env: &Env) {
    if let Some(pid) = fs::read_to_string(env.root.join("daemon.pid")).ok().and_then(|t| t.trim().parse::<i32>().ok()) {
        let _ = tracewise_platform::signal::terminate_daemon(pid);
    }
}
