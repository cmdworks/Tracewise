use crate::util::{export_destination, normalize_and_validate_path, reasons_mode_for, record_export};
use std::fs;
use std::path::{Path, PathBuf};
use tracewise_core::db::Database;
use tracewise_core::model::{Actor, CommandRecord, Intent};
use tracewise_core::ScanOptions;
use tracewise_git::{get_git_info, walk_baseline_files, RepoPaths};

pub use tracewise_shim::{detect_write_targets, find_real_tool, is_read_only, is_tracewise_bin, is_tracewise_bin_name, WriteTargets};

pub const SHIM_TOOLS: &[&str] = &["sed", "mv", "cp", "rm", "tee"];

pub fn try_fast_read_only_shim() -> bool {
    tracewise_platform::process::try_fast_read_only_sed_argv(|shim_dir| find_real_tool("sed", shim_dir))
}

pub fn strip_tracewise_why(args: &[String]) -> (Vec<String>, Option<String>) {
    let mut clean_args = Vec::new();
    let mut why = None;
    for a in args {
        if let Some(w) = a.strip_prefix("--tracewise-why=") {
            why = Some(w.to_string());
        } else if a == "--tracewise-why" {
            continue;
        } else {
            clean_args.push(a.clone());
        }
    }
    (clean_args, why)
}

pub fn exec_real_tool(real_tool: &Path, args: &[String]) -> ! {
    tracewise_platform::process::fast_exec(real_tool, args, None, Some(("TRACEWISE_SHIM_DEPTH", "1")), None);
}

pub fn normalize_target_paths(work_tree: &Path, paths: &[PathBuf]) -> Vec<PathBuf> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| work_tree.to_path_buf());
    let mut rel_paths = Vec::new();
    for p in paths {
        let full = if p.is_absolute() { p.clone() } else { cwd.join(p) };
        if let Some(safe_rel) = normalize_and_validate_path(&full.to_string_lossy(), work_tree) {
            let rel = PathBuf::from(safe_rel);
            if !rel_paths.contains(&rel) {
                rel_paths.push(rel);
            }
        }
    }
    rel_paths
}

pub fn log_shim_error(err: &str) {
    let tracewise_home = std::env::var_os("TRACEWISE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".tracewise")));
    if let Some(home) = tracewise_home {
        let _ = fs::create_dir_all(&home);
        let log_file = home.join("shim.log");
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_file) {
            use std::io::Write;
            let now = tracewise_core::clock::now().to_rfc3339();
            let _ = writeln!(f, "[{}] {}", now, err);
        }
    }
}

fn check_shim_locks(db: &Database, rel_paths: &[PathBuf], actor: &Actor) {
    for p in rel_paths {
        let p_str = p.to_string_lossy().to_string();
        if let Ok(Some(lock)) = db.get_lock_for_path(&p_str) {
            let is_same_agent =
                actor.kind == "agent" && actor.name == lock.holder.name && actor.session.is_some() && actor.session == lock.holder.session;

            if !is_same_agent {
                let holder_name = lock.holder.name.as_deref().unwrap_or("another session");
                if actor.kind != "human" {
                    eprintln!("tracewise: refused: {} is held by {}", p_str, holder_name);
                    std::process::exit(1);
                } else {
                    eprintln!("tracewise: warning: {} is held by {}", p_str, holder_name);
                }
            }
        }
    }
}

fn check_shim_reasons(db: &Database, work_tree: &Path, actor: &Actor, rel_paths: &[PathBuf]) {
    let reasons_mode = reasons_mode_for(db, work_tree, actor);
    let mut has_explicit_why = false;
    if let Ok(why_text) = std::env::var("TRACEWISE_WHY") {
        if !why_text.trim().is_empty() {
            has_explicit_why = true;
        }
    }

    if !has_explicit_why && reasons_mode.should_enforce(actor) {
        if rel_paths.is_empty() {
            eprintln!("tracewise: no intent declared; run tracewise intent add --why \"<why>\" <files>");
            std::process::exit(1);
        }
        for p in rel_paths {
            let p_str = p.to_string_lossy().to_string();
            let covered = actor
                .name
                .as_deref()
                .and_then(|agent_name| db.find_covering_intent(agent_name, actor.session.as_deref(), &p_str).ok().flatten());
            if covered.is_none() {
                eprintln!("tracewise: no intent declared for {}; run tracewise intent add --why \"<why>\" {}", p_str, p_str);
                std::process::exit(1);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn record_shim_execution(
    db: &mut Database,
    repo_paths: &RepoPaths,
    actor: &Actor,
    tool: &str,
    clean_args: &[String],
    targets: &WriteTargets,
    start_time: &str,
    end_time: &str,
    exit_code: i32,
) {
    let (branch, head) = get_git_info(repo_paths);
    let scan_opts = ScanOptions {
        source: "shim".to_string(),
        actor: Some(actor.clone()),
        confidence: Some("exact".to_string()),
        predicted_paths: None,
        window_start: Some(start_time.to_string()),
        ..Default::default()
    };

    let change_ids = match targets {
        WriteTargets::Paths(paths) => {
            let rel_paths = normalize_target_paths(&repo_paths.work_tree, paths);
            tracewise_core::scan::scan_specific_paths_with_db(
                &repo_paths.work_tree,
                &repo_paths.data_dir(),
                &branch,
                &head,
                &scan_opts,
                &rel_paths,
                db,
            )
            .unwrap_or_default()
        }
        WriteTargets::AnyInCwd => {
            let files = walk_baseline_files(&repo_paths.work_tree);
            tracewise_core::scan::scan_repository_with_db(
                &repo_paths.work_tree,
                &repo_paths.data_dir(),
                &branch,
                &head,
                &scan_opts,
                &files,
                db,
            )
            .unwrap_or_default()
        }
        WriteTargets::None => Vec::new(),
    };

    if let Ok(why_text) = std::env::var("TRACEWISE_WHY") {
        if !why_text.trim().is_empty() {
            let intent_id = ulid::Ulid::new().to_string();
            let intent = Intent {
                id: intent_id.clone(),
                kind: "shim".to_string(),
                actor: actor.clone(),
                paths: vec!["*".to_string()],
                why: why_text.clone(),
                task: None,
                created: start_time.to_string(),
                expires: (tracewise_core::clock::now() + chrono::Duration::hours(4)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                ended: 0,
            };
            let _ = db.insert_intent(&intent);
            for cid in &change_ids {
                let _ = db.link_intent_to_change(cid, &intent_id);
            }
            let _ = db.end_intent_by_id(&intent_id);
        }
    } else if let Some(ref agent_name) = actor.name {
        for cid in &change_ids {
            if let Ok(Some(ch)) = db.get_change(cid) {
                if let Ok(Some(intent)) = db.find_covering_intent(agent_name, actor.session.as_deref(), &ch.path) {
                    let _ = db.link_intent_to_change(cid, &intent.id);
                }
            }
        }
    }

    let reasons_mode = reasons_mode_for(db, &repo_paths.work_tree, actor);
    if reasons_mode.should_warn(actor) {
        for cid in &change_ids {
            if let Ok(Some(ch)) = db.get_change(cid) {
                if ch.intent.is_none() {
                    eprintln!("tracewise: {} changed ({}) without a reason. Add one: tracewise note {} \"<why>\"", ch.path, ch.id, ch.id);
                }
            }
        }
    }

    let cmd_id = ulid::Ulid::new().to_string();
    let cmd_record = CommandRecord {
        id: cmd_id.clone(),
        kind: "shim".to_string(),
        text: if clean_args.is_empty() { tool.to_string() } else { format!("{} {}", tool, clean_args.join(" ")) },
        cwd: Some(repo_paths.work_tree.to_string_lossy().to_string()),
        actor: actor.clone(),
        start: start_time.to_string(),
        end: end_time.to_string(),
        exit_code,
        changes: change_ids.clone(),
    };
    let _ = db.insert_command(&cmd_record);
    let _ = db.link_changes_to_command(&change_ids, &cmd_id);
}

pub fn run_mutating_shim(tool: &str, real_tool: &Path, clean_args: &[String], targets: WriteTargets) -> ! {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let repo_paths = match RepoPaths::discover(&current_dir) {
        Some(r) => r,
        None => {
            exec_real_tool(real_tool, clean_args);
        }
    };

    let db_path = repo_paths.data_dir().join("tracewise.db");
    let mut db = match Database::open_hot(&db_path) {
        Ok(d) => d,
        Err(e) => {
            log_shim_error(&format!("failed to open db {}: {e}", db_path.display()));
            exec_real_tool(real_tool, clean_args);
        }
    };

    let who = tracewise_proc::who(None, None);
    let actor = Actor { kind: who.kind.clone(), name: if who.kind == "human" { None } else { Some(who.name) }, session: who.session };

    if actor.kind == "agent" {
        if let Some(dest) = export_destination(&repo_paths.work_tree, &current_dir, tool, clean_args) {
            eprintln!(
                "tracewise: copying out of the repository to {}: this copy is outside the repository and not tracked",
                dest.display()
            );
            record_export(&db, &actor, tool, clean_args, &dest);
        }
    }

    let had_paths = matches!(targets, WriteTargets::Paths(ref paths) if !paths.is_empty());
    let rel_paths = match targets {
        WriteTargets::Paths(ref paths) => normalize_target_paths(&repo_paths.work_tree, paths),
        WriteTargets::AnyInCwd => Vec::new(),
        WriteTargets::None => Vec::new(),
    };

    if had_paths && rel_paths.is_empty() {
        exec_real_tool(real_tool, clean_args);
    }

    check_shim_locks(&db, &rel_paths, &actor);
    check_shim_reasons(&db, &repo_paths.work_tree, &actor, &rel_paths);

    let start_time = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let status_res = std::process::Command::new(real_tool)
        .args(clean_args)
        .env("TRACEWISE_SHIM_DEPTH", "1")
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    let exit_code = match status_res {
        Ok(status) => status.code().unwrap_or(1),
        Err(e) => {
            log_shim_error(&format!("failed to run {}: {e}", real_tool.display()));
            1
        }
    };

    let end_time = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    record_shim_execution(&mut db, &repo_paths, &actor, tool, clean_args, &targets, &start_time, &end_time, exit_code);

    std::process::exit(exit_code);
}

pub fn run_shim(tool: &str, raw_args: &[String]) -> ! {
    let parent = tracewise_platform::process::exe(tracewise_platform::process::parent_pid());
    if tracewise_shim::shim_guard_applies(
        std::env::var_os("TRACEWISE_SHIM_DEPTH").is_some(),
        parent.as_deref(),
        std::env::current_exe().ok().as_deref(),
    ) {
        let fallback = if Path::new(&format!("/usr/bin/{tool}")).is_file() {
            PathBuf::from(format!("/usr/bin/{tool}"))
        } else {
            PathBuf::from(format!("/bin/{tool}"))
        };
        let (clean_args, _) = strip_tracewise_why(raw_args);
        exec_real_tool(&fallback, &clean_args);
    }

    let (clean_args, why) = strip_tracewise_why(raw_args);
    if let Some(ref w) = why {
        if std::env::var_os("TRACEWISE_WHY").is_none() {
            std::env::set_var("TRACEWISE_WHY", w);
        }
    }

    let shim_dir = std::env::args().next().and_then(|a| {
        let p = PathBuf::from(a);
        p.parent().map(|d| d.to_path_buf())
    });

    let real_tool = find_real_tool(tool, shim_dir.as_deref());

    if is_read_only(tool, &clean_args) {
        exec_real_tool(&real_tool, &clean_args);
    }

    let targets = detect_write_targets(tool, &clean_args);
    run_mutating_shim(tool, &real_tool, &clean_args, targets);
}

#[cfg(test)]
#[path = "test_shim_detect.rs"]
mod tests;
