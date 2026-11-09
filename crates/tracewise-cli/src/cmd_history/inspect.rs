//! Inspection commands: log, show, blame, why, diff, and display helpers.
use super::*;

/// The reason text behind a change's `intent` id (the id alone is not an explanation).
pub(super) fn reason_text(db: Option<&Database>, c: &tracewise_core::model::Change) -> Option<String> {
    let iid = c.intent.as_deref()?;
    db?.get_intent(iid).ok().flatten().map(|i| i.why)
}

/// Ids of changes that created or edited a file but have no stored version: recorded by hash only (binary, large, a link, or
/// a file holding a secret). One lookup for all of them, so `log` stays fast.
fn hash_only_ids(blobs: &BlobStore, changes: &[Change]) -> HashSet<String> {
    let candidates: Vec<&Change> =
        changes.iter().filter(|c| c.hunks.is_empty() && matches!(c.op.as_str(), "create" | "modify") && c.after.is_some()).collect();
    let hashes: Vec<String> = candidates.iter().filter_map(|c| c.after.clone()).collect();
    let missing: HashSet<String> = blobs.missing(&hashes).into_iter().collect();
    candidates.into_iter().filter(|c| c.after.as_ref().is_some_and(|h| missing.contains(h))).map(|c| c.id.clone()).collect()
}

/// What `trail log` shows besides the path and limit: `--actor`, `--since`, `--grep` filter; `--group` clusters by task.
pub(crate) struct LogFilter {
    pub actor: Option<String>,
    pub since: Option<String>,
    pub grep: Option<String>,
    pub after: Option<String>,
    pub group: bool,
}

impl LogFilter {
    fn narrows(&self) -> bool {
        self.actor.is_some() || self.since.is_some() || self.grep.is_some() || self.after.is_some()
    }

    fn keeps(&self, db: Option<&Database>, c: &Change, cutoff: &Option<String>) -> bool {
        self.after.as_deref().is_none_or(|a| c.id.as_str() > a)
            && self.actor.as_deref().is_none_or(|a| c.actor.name.as_deref() == Some(a) || c.actor.kind == a)
            && cutoff.as_deref().is_none_or(|t| c.time.start.as_str() >= t)
            && self.grep.as_deref().is_none_or(|g| {
                let g = g.to_lowercase();
                c.path.to_lowercase().contains(&g) || reason_text(db, c).is_some_and(|w| w.to_lowercase().contains(&g))
            })
    }
}

/// The task a change belongs to: its intent, else its session, else none. Used by `log --group`.
fn group_key(c: &Change) -> String {
    match (&c.intent, &c.actor.session) {
        (Some(i), _) => format!("intent:{i}"),
        (None, Some(s)) => format!("session:{s}"),
        _ => "none".to_string(),
    }
}

/// Groups in order of first appearance; inside a group the listing order is kept.
fn grouped(changes: Vec<Change>) -> Vec<Change> {
    let mut order: Vec<String> = Vec::new();
    for c in &changes {
        let k = group_key(c);
        if !order.contains(&k) {
            order.push(k);
        }
    }
    let mut out = Vec::with_capacity(changes.len());
    for k in &order {
        out.extend(changes.iter().filter(|c| &group_key(c) == k).cloned());
    }
    out
}

pub(crate) fn run_log(json: bool, repo_paths: &RepoPaths, path: Option<String>, limit: Option<usize>, filter: LogFilter) {
    let db_path = repo_paths.data_dir().join("trail.db");
    let db = Database::open(&db_path).ok();
    let mut changes = if db_path.is_file() {
        // a filter is applied before the limit, so `--limit 5 --actor x` is the last five of x
        let fetch_limit = if filter.narrows() { None } else { limit };
        match Database::open(&db_path).and_then(|d| d.list_changes(path.as_deref(), fetch_limit)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("trail: cannot read the history: {e}");
                process::exit(1);
            }
        }
    } else {
        Vec::new()
    };
    if filter.narrows() {
        let cutoff = filter.since.as_deref().and_then(parse_since).map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        changes.retain(|c| filter.keeps(db.as_ref(), c, &cutoff));
        if let Some(n) = limit {
            changes.truncate(n);
        }
    }
    if filter.group {
        changes = grouped(changes);
    }
    let hash_only = hash_only_ids(&BlobStore::new(&repo_paths.data_dir()), &changes);

    if json {
        let rows: Vec<serde_json::Value> = changes
            .iter()
            .map(|c| {
                let mut v = serde_json::to_value(c).unwrap_or_default();
                v["why"] = serde_json::json!(reason_text(db.as_ref(), c));
                v["hash_only"] = serde_json::json!(hash_only.contains(&c.id));
                if filter.group {
                    v["group"] = serde_json::json!(group_key(c));
                }
                v
            })
            .collect();
        println!("{}", serde_json::to_string(&rows).unwrap_or_default());
        return;
    }
    let mut current = None;
    for c in &changes {
        if filter.group && current.as_ref() != Some(&group_key(c)) {
            let k = group_key(c);
            let n = changes.iter().filter(|o| group_key(o) == k).count();
            let why = reason_text(db.as_ref(), c).map(|w| format!(" \"{w}\"")).unwrap_or_default();
            println!("== {k}{why} ({n} change{})", if n == 1 { "" } else { "s" });
            current = Some(k);
        }
        let why = reason_text(db.as_ref(), c);
        println!(
            "{} {} {:<8} {} {}{}{}{}",
            c.id,
            c.time.start,
            c.op,
            c.path,
            actor_label(&c.actor, &c.confidence),
            if hash_only.contains(&c.id) { "  (hash only)" } else { "" },
            why.map(|w| format!("  \"{w}\"")).unwrap_or_default(),
            db.as_ref().and_then(|db| overlap_note(db, c)).map(|n| format!("  [{n}]")).unwrap_or_default()
        );
    }
}

/// One file at a glance: who holds it now (lock) and its latest changes with actor, confidence and reason.
pub(crate) fn run_file(json: bool, repo_paths: &RepoPaths, path: String, limit: usize) {
    // `./a.txt` and an absolute path name the same file as `a.txt`
    let path = crate::util::normalize_and_validate_path(&path, &repo_paths.work_tree).unwrap_or(path);
    let db = match Database::open(&repo_paths.data_dir().join("trail.db")) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("trail: cannot read the history: {e}");
            process::exit(1);
        }
    };
    let changes = db.list_changes(Some(&path), Some(limit)).unwrap_or_default();
    let lock = db.get_lock_for_path(&path).ok().flatten();
    let hash_only = hash_only_ids(&BlobStore::new(&repo_paths.data_dir()), &changes);
    if json {
        let rows: Vec<serde_json::Value> = changes
            .iter()
            .map(|c| {
                let mut v = serde_json::to_value(c).unwrap_or_default();
                v["why"] = serde_json::json!(reason_text(Some(&db), c));
                v["hash_only"] = serde_json::json!(hash_only.contains(&c.id));
                v
            })
            .collect();
        println!("{}", serde_json::json!({"path": path, "lock": lock, "changes": rows}));
        return;
    }
    println!("{path}");
    match &lock {
        Some(l) => println!("  locked by {} until {}", l.holder, l.expires),
        None => println!("  not locked"),
    }
    if changes.is_empty() {
        println!("  no recorded changes");
    }
    for c in &changes {
        let why = reason_text(Some(&db), c).map(|w| format!("  \"{w}\"")).unwrap_or_default();
        let note = if hash_only.contains(&c.id) { "  (hash only)" } else { "" };
        println!("  {} {:<8} {}{note}{why}", c.time.start, c.op, actor_label(&c.actor, &c.confidence));
    }
}

pub(crate) fn run_show(json: bool, repo_paths: &RepoPaths, id: String) {
    let db_path = repo_paths.data_dir().join("trail.db");
    if !db_path.is_file() {
        eprintln!("trail: change not found: {id}");
        process::exit(1);
    }

    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(_) => {
            eprintln!("trail: change not found: {id}");
            process::exit(1);
        }
    };

    let blobs = BlobStore::new(&repo_paths.data_dir());
    if let Ok(Some(change)) = db.get_change(&id) {
        let before_bytes = stored_text(&blobs, change.before.as_deref(), &change);
        let after_bytes = stored_text(&blobs, change.after.as_deref(), &change);
        let diff_text = format_unified_diff(before_bytes.as_deref(), after_bytes.as_deref(), &change.path);
        let hash_only = hash_only_ids(&blobs, std::slice::from_ref(&change)).contains(&change.id);

        if json {
            let mut val = serde_json::to_value(&change).unwrap();
            if let serde_json::Value::Object(ref mut map) = val {
                map.insert("diff".to_string(), serde_json::Value::String(diff_text));
                map.insert("why".to_string(), serde_json::json!(reason_text(Some(&db), &change)));
                map.insert("hash_only".to_string(), serde_json::json!(hash_only));
            }
            println!("{}", val);
        } else {
            println!("change {}", change.id);
            println!("Author: {} ({})", change.actor.kind, change.confidence);
            println!("Date:   {}", change.time.start);
            println!("Path:   {}", change.path);
            if let Some(why) = reason_text(Some(&db), &change) {
                println!("Why:    {why}");
            }
            if hash_only {
                println!("Note:   recorded by hash only (binary, large, a link or a file with a secret): no stored version");
            }
            println!("\n{}", diff_text);
        }
    } else {
        eprintln!("trail: change not found: {id}");
        process::exit(1);
    }
}

/// The lines of `path` as they were right after change `at`, with the change that owns each line (U5). Replays the file's changes
/// from the start, as an import does, so the answer for the newest change equals the live blame.
pub(crate) fn replay_blame(
    changes: &[tracewise_core::model::Change],
    at: &str,
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<(String, Vec<String>), String> {
    let mut map = tracewise_core::linemap::LineMap::new();
    let mut ids: Vec<&str> = Vec::new();
    let mut text = String::new();
    for c in changes {
        if c.op == "rename" || c.old_path.is_some() {
            return Err("this file was renamed; `--at` does not follow renames yet".to_string());
        }
        ids.push(&c.id);
        let seq = ids.len() as u32;
        match c.op.as_str() {
            "delete" => {
                map = tracewise_core::linemap::LineMap::new();
                text.clear();
            }
            _ => {
                let new = match c.after.as_deref() {
                    Some(h) => read(h).ok_or_else(|| format!("the stored version of change {} is not available", c.id))?,
                    None => return Err(format!("change {} kept only a hash of this file, not its text", c.id)),
                };
                // the change's own `before` is the baseline text for a file Tracewise saw first in this change
                let old = c.before.as_deref().and_then(read).unwrap_or_else(|| text.clone());
                map = map.apply_diff(Some(&old), Some(&new), seq);
                text = new;
            }
        }
        if c.id == at {
            let lines = map
                .to_line_seqs()
                .into_iter()
                .map(|s| if s == 0 { "baseline".to_string() } else { ids[s as usize - 1].to_string() })
                .collect();
            return Ok((text, lines));
        }
    }
    Err(format!("change {at} is not a change of this file"))
}

pub(crate) fn run_blame(json: bool, repo_paths: &RepoPaths, path: String, at: Option<String>) {
    let db_path = repo_paths.data_dir().join("trail.db");
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("trail: blame failed: {e}");
            process::exit(1);
        }
    };

    let (content, line_entries): (String, std::collections::HashMap<usize, String>) = if let Some(at) = at {
        let mut changes = db.list_changes(Some(&path), None).unwrap_or_default();
        changes.retain(|c| c.op != "baseline");
        changes.sort_by(|a, b| (&a.time.start, &a.id).cmp(&(&b.time.start, &b.id)));
        let blobs = BlobStore::new(&repo_paths.data_dir());
        let read = |h: &str| blobs.read(h).ok().and_then(|b| String::from_utf8(b).ok());
        match replay_blame(&changes, &at, &read) {
            Ok((text, ids)) => (text, ids.into_iter().enumerate().map(|(i, id)| (i + 1, id)).collect()),
            Err(e) => {
                eprintln!("trail: blame --at failed: {e}");
                process::exit(1);
            }
        }
    } else {
        let file_path = repo_paths.work_tree.join(&path);
        let content = match fs::read(&file_path).map(|b| String::from_utf8_lossy(&b).into_owned()) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("trail: cannot read file: {e}");
                process::exit(1);
            }
        };
        (content, db.get_file_lines(&path).unwrap_or_default().into_iter().collect())
    };
    // many lines share one change: look each change (and its reason) up once
    let mut by_change: std::collections::HashMap<String, (tracewise_core::model::Actor, String, Option<String>)> =
        std::collections::HashMap::new();
    let mut entries: Vec<serde_json::Value> = Vec::new();

    for (idx, line_text) in content.lines().enumerate() {
        let line_num = idx + 1;
        let change_id = line_entries.get(&line_num).map(String::as_str).unwrap_or("");

        let (actor, time, why) = by_change
            .entry(change_id.to_string())
            .or_insert_with(|| {
                if let Ok(Some(ch)) = db.get_change(change_id) {
                    let why_str = if let Some(ref iid) = ch.intent {
                        db.get_intent(iid).ok().flatten().map(|i| i.why).or_else(|| ch.intent.clone())
                    } else {
                        None
                    };
                    (ch.actor, ch.time.start, why_str)
                } else if change_id.is_empty() || change_id == "baseline" {
                    (tracewise_core::model::Actor::baseline(), String::new(), None)
                } else {
                    (tracewise_core::model::Actor::unknown(), String::new(), None)
                }
            })
            .clone();

        let entry = serde_json::json!({
            "line": line_num,
            "text": line_text,
            "change": change_id,
            "actor": actor,
            "time": time,
            "why": why,
        });
        entries.push(entry);
    }

    if entries.iter().any(|e| e["actor"]["kind"] == "baseline") {
        let authors = git_line_authors(&repo_paths.work_tree, &path);
        for e in entries.iter_mut().filter(|e| e["actor"]["kind"] == "baseline") {
            let i = e["line"].as_u64().unwrap_or(1) as usize - 1;
            e["git_author"] = authors.get(i).cloned().flatten().into();
        }
    }

    if json {
        println!("{}", serde_json::to_string(&entries).unwrap_or_default());
    } else {
        for e in &entries {
            let actor: tracewise_core::model::Actor =
                serde_json::from_value(e["actor"].clone()).unwrap_or_else(|_| tracewise_core::model::Actor::unknown());
            println!(
                "{:>4} {} {:<14} {} | {}{}",
                e["line"].as_u64().unwrap_or(0),
                e["change"].as_str().unwrap_or(""),
                actor_label(&actor, ""),
                e["time"].as_str().unwrap_or(""),
                e["text"].as_str().unwrap_or(""),
                e["why"]
                    .as_str()
                    .map(|w| format!("   <- \"{w}\""))
                    .or_else(|| e["git_author"].as_str().map(|a| format!("   (git: {a})")))
                    .unwrap_or_default()
            );
        }
    }
}

fn baseline_change_for(path: &str) -> tracewise_core::model::Change {
    tracewise_core::model::Change {
        id: "baseline".to_string(),
        time: tracewise_core::model::TimeRange { start: String::new(), end: String::new() },
        path: path.to_string(),
        op: "baseline".to_string(),
        old_path: None,
        hunks: Vec::new(),
        before: None,
        after: None,
        actor: tracewise_core::model::Actor::baseline(),
        candidates: Vec::new(),
        confidence: "exact".to_string(),
        source: "git".to_string(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: String::new(),
        git: tracewise_core::model::GitInfo { branch: String::new(), head: String::new() },
        prev_hash: None,
        undoes: None,
    }
}

pub(crate) fn run_why(json: bool, repo_paths: &RepoPaths, target: String) {
    let db_path = repo_paths.data_dir().join("trail.db");
    let db = match Database::open(&db_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("trail: why failed: {e}");
            process::exit(1);
        }
    };

    let parts: Vec<&str> = target.split(':').collect();
    if parts.len() != 2 {
        eprintln!("trail: why target must be <path>:<line>");
        process::exit(1);
    }
    let path = parts[0];
    let Ok(line_num) = parts[1].parse::<usize>() else {
        eprintln!("trail: invalid line number");
        process::exit(1);
    };

    let line_entries = db.get_file_lines(path).unwrap_or_default();
    let change_id = match line_entries.iter().find(|(l, _)| *l == line_num) {
        Some((_, cid)) => cid.clone(),
        None => {
            let file_path = repo_paths.work_tree.join(path);
            if fs::read(&file_path).is_ok_and(|b| line_num >= 1 && line_num <= String::from_utf8_lossy(&b).lines().count()) {
                "baseline".to_string()
            } else {
                eprintln!("trail: line not found");
                process::exit(1);
            }
        }
    };

    let change = if change_id == "baseline" {
        baseline_change_for(path)
    } else {
        match db.get_change(&change_id) {
            Ok(Some(c)) => c,
            _ => {
                eprintln!("trail: change not found");
                process::exit(1);
            }
        }
    };

    // Find earlier changes for this path
    let all_changes = db.list_changes(Some(path), None).unwrap_or_default();
    let mut history = Vec::new();
    let mut seen_current = false;
    for ch in all_changes {
        if ch.id == change.id {
            seen_current = true;
            continue;
        }
        if seen_current {
            history.push(ch);
        }
    }

    let intent_doc = if let Some(ref iid) = change.intent {
        serde_json::to_value(db.get_intent(iid).ok().flatten()).unwrap_or(serde_json::Value::Null)
    } else {
        serde_json::Value::Null
    };

    let doc = serde_json::json!({
        "line": line_num,
        "change": change,
        "intent": intent_doc,
        "history": history,
    });

    if json {
        println!("{}", doc);
    } else {
        println!(
            "Line {} in {} changed by {} at {} ({})",
            line_num,
            path,
            actor_label(&change.actor, &change.confidence),
            change.time.start,
            change.id
        );
        match intent_doc.get("why").and_then(|w| w.as_str()) {
            Some(w) => println!("Why: {w}"),
            None => println!("Why: (no reason recorded)"),
        }
        if let Some(n) = overlap_note(&db, &change) {
            println!("Overlap: {n}");
        }
        if change.actor.kind == "baseline" {
            let authors = git_line_authors(&repo_paths.work_tree, path);
            if let Some(Some(a)) = authors.get(line_num - 1) {
                println!("Git author: {a} (written before Trail started)");
            }
        }
        for h in history.iter().take(5) {
            let why =
                h.intent.as_ref().and_then(|iid| db.get_intent(iid).ok().flatten()).map(|i| format!(" \"{}\"", i.why)).unwrap_or_default();
            println!("  before: {} {} by {}{}", h.time.start, h.op, actor_label(&h.actor, ""), why);
        }
    }
}

/// The stored text of `hash`. Unreadable is damage when the change kept line hunks; otherwise it is a big or binary file with no stored content.
fn stored_text(blobs: &BlobStore, hash: Option<&str>, change: &tracewise_core::model::Change) -> Option<Vec<u8>> {
    let h = hash?;
    blobs.read(h).ok().or_else(|| {
        if !change.hunks.is_empty() {
            eprintln!("trail: the stored version {h} of {} is missing or unreadable", change.path);
            process::exit(1);
        }
        None
    })
}

pub(crate) fn run_diff(_json: bool, repo_paths: &RepoPaths, since: Option<String>, path: Option<String>) {
    crate::util::require_valid_since(since.as_deref());
    if since.is_none() && path.is_none() {
        eprintln!("trail: diff needs a <path>, or --since to look at every file changed since then");
        process::exit(2);
    }
    let exit = |e: &dyn std::fmt::Display| -> ! {
        eprintln!("trail: diff failed: {e}");
        process::exit(1)
    };
    let db = Database::open(&repo_paths.data_dir().join("trail.db")).unwrap_or_else(|e| exit(&e));
    let blobs = BlobStore::new(&repo_paths.data_dir());
    let cutoff = since.as_deref().and_then(crate::util::parse_since);

    // the named file, or every file changed since the cutoff
    let mut targets: Vec<String> = match (&path, cutoff) {
        (Some(p), _) => vec![p.clone()],
        (None, Some(t)) => db
            .list_changes_since(&t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
            .unwrap_or_else(|e| exit(&e))
            .into_iter()
            .filter(|c| c.op != "baseline")
            .map(|c| c.path)
            .collect(),
        (None, None) => Vec::new(),
    };
    targets.sort();
    targets.dedup();

    for target in targets {
        let changes: Vec<_> =
            db.list_changes(Some(&target), None).unwrap_or_else(|e| exit(&e)).into_iter().filter(|c| c.op != "baseline").collect(); // newest first
                                                                                                                                    // the state at the cutoff: what the newest change before it left, or (none before it) what the oldest change found
        let point = match cutoff {
            Some(t) => changes
                .iter()
                .find(|c| chrono::DateTime::parse_from_rfc3339(&c.time.start).is_ok_and(|s| s < t))
                .map(|c| (c, c.after.as_deref()))
                .or_else(|| changes.last().map(|c| (c, c.before.as_deref()))),
            None => changes.first().map(|c| (c, c.before.as_deref())),
        };
        let before = point.and_then(|(c, h)| stored_text(&blobs, h, c));
        let current = fs::read(repo_paths.work_tree.join(&target)).ok();
        print!("{}", format_unified_diff(before.as_deref(), current.as_deref(), &target));
    }
}

pub(crate) fn overlap_note(db: &Database, c: &tracewise_core::model::Change) -> Option<String> {
    let prior = db.overlapping_prior(c, 10).ok().flatten()?;
    let secs = match (chrono::DateTime::parse_from_rfc3339(&c.time.start), chrono::DateTime::parse_from_rfc3339(&prior.time.start)) {
        (Ok(a), Ok(b)) => (a - b).num_seconds().max(0),
        _ => 0,
    };
    Some(format!("{secs}s after {} changed it too", actor_label(&prior.actor, "")))
}

pub(crate) fn git_line_authors(work_tree: &std::path::Path, path: &str) -> Vec<Option<String>> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(work_tree)
        .args(["blame", "--porcelain", "--", path])
        .env("TRAIL_GIT_DEPTH", "1")
        .output();
    let Ok(o) = out else { return Vec::new() };
    let mut by_sha: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let (mut sha, mut author, mut day) = (String::new(), String::new(), String::new());
    let mut lines = Vec::new();
    for l in String::from_utf8_lossy(&o.stdout).lines() {
        if l.starts_with('\t') {
            let who = by_sha.entry(sha.clone()).or_insert_with(|| {
                if author == "Not Committed Yet" {
                    String::new()
                } else {
                    format!("{author}, {day}")
                }
            });
            lines.push(Some(who.clone()).filter(|w| !w.is_empty()));
        } else if let Some(a) = l.strip_prefix("author ") {
            author = a.to_string();
        } else if let Some(t) = l.strip_prefix("author-time ") {
            let secs: i64 = t.parse().unwrap_or(0);
            day = chrono::DateTime::from_timestamp(secs, 0).map(|d| d.format("%Y-%m-%d").to_string()).unwrap_or_default();
        } else if l.len() > 40 && l.as_bytes()[40] == b' ' && l[..40].bytes().all(|b| b.is_ascii_hexdigit()) {
            sha = l[..40].to_string();
        }
    }
    lines
}

pub(crate) fn actor_label(a: &tracewise_core::model::Actor, confidence: &str) -> String {
    let who = match (a.kind.as_str(), a.name.as_deref(), a.session.as_deref()) {
        ("human", _, _) => "you".to_string(),
        (_, Some(n), Some(s)) => format!("{n}/{s}"),
        (_, Some(n), None) => n.to_string(),
        (k, _, _) => k.to_string(),
    };
    if confidence.is_empty() || confidence == "exact" {
        who
    } else {
        format!("{who} ({confidence})")
    }
}

#[cfg(test)]
mod blame_at_tests {
    use super::*;
    use tracewise_core::model::{Actor, Change, GitInfo, TimeRange};

    fn ch(n: u32, op: &str, after: Option<&str>) -> Change {
        Change {
            id: format!("c{n}"),
            time: TimeRange { start: format!("2026-01-01T00:00:0{n}Z"), end: String::new() },
            path: "f".into(),
            op: op.into(),
            old_path: None,
            hunks: vec![],
            before: None,
            after: after.map(String::from),
            actor: Actor::unknown(),
            candidates: vec![],
            confidence: "unknown".into(),
            source: "scan".into(),
            command: None,
            intent: None,
            lock_conflict: None,
            device: "d".into(),
            git: GitInfo { branch: "m".into(), head: "h".into() },
            prev_hash: None,
            undoes: None,
        }
    }

    #[test]
    fn each_line_belongs_to_the_change_that_wrote_it_at_that_point() {
        let versions = [("h1", "a\nb\n"), ("h2", "a\nB\nc\n"), ("h3", "a\nB\n")];
        let read = |h: &str| versions.iter().find(|(k, _)| *k == h).map(|(_, v)| v.to_string());
        let changes = vec![ch(1, "create", Some("h1")), ch(2, "modify", Some("h2")), ch(3, "modify", Some("h3"))];
        let (t, ids) = replay_blame(&changes, "c1", &read).unwrap();
        assert_eq!((t.as_str(), ids), ("a\nb\n", vec!["c1".to_string(), "c1".into()]));
        let (_, ids) = replay_blame(&changes, "c2", &read).unwrap();
        assert_eq!(ids, ["c1", "c2", "c2"]);
        let (t, ids) = replay_blame(&changes, "c3", &read).unwrap();
        assert_eq!((t.as_str(), ids), ("a\nB\n", vec!["c1".to_string(), "c2".into()]));
        assert!(replay_blame(&changes, "nope", &read).is_err());
        assert!(replay_blame(&changes, "c1", &|_| None).is_err());
        let mut renamed = changes.clone();
        renamed[1].old_path = Some("g".into());
        assert!(replay_blame(&renamed, "c3", &read).unwrap_err().contains("renamed"));
    }
}
