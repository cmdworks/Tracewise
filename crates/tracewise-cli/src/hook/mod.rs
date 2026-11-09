pub mod adapter;
pub mod antigravity;
pub mod claude;
pub mod events;
pub mod helpers;

use std::io::Read;
use tracewise_core::db::Database;
use tracewise_core::model::Actor;
use tracewise_git::RepoPaths;

#[derive(Debug, serde::Deserialize, Default)]
pub struct HookInput {
    pub agent: Option<String>,
    pub session: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    pub command: Option<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct HookOutput {
    pub decision: &'static str,
    pub reason: Option<String>,
    pub notice: Option<String>,
}

pub struct HookContext<'a> {
    pub db: &'a Database,
    pub repo_paths: &'a RepoPaths,
    pub actor: &'a Actor,
    pub agent: &'a str,
    pub session: &'a str,
    pub now: &'a str,
    pub ttl: u64,
    pub profile: tracewise_core::profile::ProfileConfig,
}

pub fn handle_hook(event: &str, from: Option<&str>, record: Option<&std::path::Path>) {
    let mut stdin_str = String::new();
    let _ = std::io::stdin().read_to_string(&mut stdin_str);
    if let Some(dir) = record {
        let _ = std::fs::create_dir_all(dir);
        let name = if event.is_empty() { "payload" } else { event };
        let _ = std::fs::write(dir.join(format!("{name}.json")), &stdin_str);
    }

    if let Some(agent @ ("claude" | "codex" | "cursor" | "gemini")) = from {
        claude::handle_claude_hook(agent, event, &stdin_str);
        return;
    }
    if from == Some("antigravity") {
        antigravity::handle_antigravity_hook(event, &stdin_str);
        return;
    }

    if let Some(name) = from {
        let work_tree = std::env::current_dir().ok().and_then(|d| RepoPaths::discover(&d)).map(|r| r.work_tree);
        match adapter::find(name, work_tree.as_deref()) {
            Ok(Some((a, _))) => {
                adapter::handle_file_hook(&a, event, &stdin_str);
                return;
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("tracewise hook fail-open: {e}");
                println!("{{}}");
                return;
            }
        }
    }

    let input: HookInput = serde_json::from_str(&stdin_str).unwrap_or_default();

    let output = execute_hook(event, input).unwrap_or_else(|e| {
        eprintln!("tracewise hook fail-open: {e}");
        HookOutput { decision: "allow", reason: None, notice: None }
    });

    println!("{}", serde_json::to_string(&output).unwrap());
}

pub fn execute_hook(event: &str, input: HookInput) -> Result<HookOutput, String> {
    let current_dir = std::env::current_dir().map_err(|e| e.to_string())?;
    let repo_paths = match RepoPaths::discover(&current_dir) {
        Some(p) => p,
        None => {
            return Ok(HookOutput { decision: "allow", reason: None, notice: None });
        }
    };
    let data_dir = repo_paths.data_dir();
    let db_path = data_dir.join("tracewise.db");
    if !db_path.exists() {
        return Ok(HookOutput { decision: "allow", reason: None, notice: None });
    }

    let agent = input.agent.clone().unwrap_or_else(|| "unknown".to_string());
    let session = input.session.clone().unwrap_or_else(|| "default".to_string());
    let actor = Actor::agent(&agent, &session);

    let db = match Database::open_hot(&db_path) {
        Ok(d) => d,
        Err(_) => {
            if event == "post-edit" {
                helpers::spool_post_edit(&repo_paths.work_tree, &data_dir, &actor, &input.paths, &repo_paths);
            }
            return Ok(HookOutput { decision: "allow", reason: None, notice: None });
        }
    };
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let ttl = crate::util::lock_ttl_secs();

    let _ = db.clean_expired_locks(&repo_paths.work_tree);
    let _ = db.renew_session_locks(&session, ttl);

    let profile = tracewise_core::profile::ProfileConfig::from_root(&repo_paths.work_tree);

    let ctx = HookContext { db: &db, repo_paths: &repo_paths, actor: &actor, agent: &agent, session: &session, now: &now, ttl, profile };

    dispatch_event(event, &ctx, input)
}

fn dispatch_event(event: &str, ctx: &HookContext, input: HookInput) -> Result<HookOutput, String> {
    match event {
        "pre-edit" => events::pre_edit::handle(ctx, input),
        "post-edit" => events::post_edit::handle(ctx, input),
        "pre-command" => events::pre_command::handle(ctx, input),
        "post-command" => events::post_command::handle(ctx),
        "stop" => events::stop::handle(ctx),
        "prompt" => events::prompt::handle(ctx),
        "session-end" => events::session_end::handle(ctx),
        _ => Ok(HookOutput { decision: "allow", reason: None, notice: None }),
    }
}
