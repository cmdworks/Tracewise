//! `tracewise prune`: removes the history of ignored (or chosen) paths.
use super::*;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, IsTerminal, Write};

pub(crate) struct PruneArgs {
    pub paths: Vec<String>,
    pub older_than: Option<String>,
    pub dry_run: bool,
    pub yes: bool,
    pub backup: Option<String>,
}

fn fail(msg: &str, code: i32) -> ! {
    eprintln!("tracewise: {msg}");
    process::exit(code);
}

fn db_bytes(data_dir: &Path) -> u64 {
    ["tracewise.db", "tracewise.db-wal"].iter().filter_map(|f| fs::metadata(data_dir.join(f)).ok()).map(|m| m.len()).sum()
}

/// How many records are under paths that are ignored today (what `tracewise prune` would remove), for `tracewise doctor`'s alert.
pub(crate) fn candidates(repo_paths: &RepoPaths) -> usize {
    let Ok(db) = Database::open_hot(&repo_paths.data_dir().join("tracewise.db")) else { return 0 };
    let Ok(keys) = db.change_keys() else { return 0 };
    let mut rules = tracewise_core::ignore_rules::IgnoreRules::load(&repo_paths.work_tree);
    let mut ignored: HashMap<&str, bool> = HashMap::new();
    keys.iter().filter(|(_, path, _)| *ignored.entry(path.as_str()).or_insert_with(|| rules.is_ignored(Path::new(path)))).count()
}

pub(crate) fn run_prune(json: bool, repo_paths: &RepoPaths, args: PruneArgs) {
    // a dry run only reads, so an agent may look (the doctor alert points at it)
    if !args.dry_run && tracewise_core::model::Actor::from_env().is_some_and(|a| a.kind == "agent") {
        fail("refused: agents cannot prune history", 1);
    }
    let cutoff = args.older_than.as_deref().map(|v| match tracewise_core::db::parse_retention_duration(v) {
        Some(d) => (tracewise_core::clock::now() - d).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        None => fail(&format!("invalid --older-than '{v}': use a duration like 30d or 12w"), 2),
    });

    let data_dir = repo_paths.data_dir();
    let db = match Database::open(&data_dir.join("tracewise.db")) {
        Ok(d) => d,
        Err(e) => fail(&format!("failed to open db: {e}"), 1),
    };
    let mut rules = tracewise_core::ignore_rules::IgnoreRules::load(&repo_paths.work_tree);
    let mut ignored: HashMap<String, bool> = HashMap::new();
    let keys = db.change_keys().unwrap_or_else(|e| fail(&format!("prune failed: {e}"), 1));
    let mut ids: HashSet<String> = HashSet::new();
    for (id, path, start) in keys {
        if cutoff.as_deref().is_some_and(|c| start.as_str() >= c) {
            continue;
        }
        let wanted = if args.paths.is_empty() {
            *ignored.entry(path.clone()).or_insert_with(|| rules.is_ignored(Path::new(&path)))
        } else {
            args.paths.iter().any(|g| tracewise_core::glob_match(g, &path))
        };
        if wanted {
            ids.insert(id);
        }
    }

    // With sharing on, the remote must already hold a record before it is removed here
    let mut skipped_unpushed = 0;
    if tracewise_core::share::read_tracewiseconfig(&repo_paths.work_tree).is_some() {
        let pushed = db.pushed_change_ids().unwrap_or_default();
        let before = ids.len();
        ids.retain(|id| pushed.contains(id));
        skipped_unpushed = before - ids.len();
    }

    let selector = if args.paths.is_empty() { "ignored".to_string() } else { format!("--path {}", args.paths.join(" ")) };
    let selector = match &args.older_than {
        Some(v) => format!("{selector} --older-than {v}"),
        None => selector,
    };
    let actor = get_human_actor();
    let actor_name = actor.name.clone().unwrap_or_else(|| actor.kind.clone());
    let forget_state = args.paths.is_empty();

    let preview =
        db.prune_changes(&ids, &actor_name, &selector, forget_state, true).unwrap_or_else(|e| fail(&format!("prune failed: {e}"), 1));
    let bytes_before = db_bytes(&data_dir);
    let mut report = preview.clone();
    if !args.dry_run && preview.removed > 0 {
        if !args.yes {
            if !std::io::stdin().is_terminal() {
                fail("pass --yes to prune without a terminal", 1);
            }
            eprint!("Remove {} record(s) in {} run(s) from the history? This cannot be undone. [y/N] ", preview.removed, preview.gaps);
            let _ = std::io::stderr().flush();
            let mut answer = String::new();
            let _ = std::io::stdin().lock().read_line(&mut answer);
            if !answer.trim().eq_ignore_ascii_case("y") {
                fail("not pruned", 1);
            }
        }
        if let Some(file) = &args.backup {
            if Path::new(file).exists() {
                fail(&format!("backup file {file} already exists"), 1);
            }
            if let Err(e) = db.backup_to(Path::new(file)) {
                fail(&format!("backup failed, nothing was removed: {e}"), 1);
            }
        }
        report = db
            .prune_changes(&ids, &actor_name, &selector, forget_state, false)
            .unwrap_or_else(|e| fail(&format!("prune failed, nothing was removed: {e}"), 1));
        if let Err(e) = db.compact() {
            eprintln!("tracewise: pruned, but compacting the database failed: {e}");
        }
    }
    let bytes_after = if args.dry_run { bytes_before } else { db_bytes(&data_dir) };

    if json {
        println!(
            "{}",
            serde_json::json!({
                "removed": report.removed, "skipped_unpushed": skipped_unpushed, "gaps": report.gaps,
                "bytes_before": bytes_before, "bytes_after": bytes_after, "dry_run": args.dry_run, "folders": report.folders,
            })
        );
        return;
    }
    let verb = if args.dry_run { "Would remove" } else { "Removed" };
    println!("{verb} {} record(s) in {} run(s) ({selector})", report.removed, report.gaps);
    let mut folders: Vec<_> = report.folders.iter().collect();
    folders.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (name, n) in folders.iter().take(10) {
        println!("  {n:>8}  {name}");
    }
    if skipped_unpushed > 0 {
        println!("Skipped {skipped_unpushed} record(s) not pushed to the remote yet");
    }
    if !args.dry_run && report.removed > 0 {
        println!("Database: {bytes_before} bytes -> {bytes_after} bytes");
    }
}
