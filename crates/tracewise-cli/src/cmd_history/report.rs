//! `trail report`: one task (a session, an intent, an actor) as Markdown, for a code review or a hand-off.
use super::inspect::{actor_label, reason_text};
use super::revert::Target;
use super::*;

pub(crate) fn run_report(json: bool, repo_paths: &RepoPaths, target: Target, since: Option<String>) {
    let db = match Database::open(&repo_paths.data_dir().join("trail.db")) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("trail: cannot read the history: {e}");
            process::exit(1);
        }
    };
    let cutoff = since.as_deref().and_then(parse_since).map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
    let mut changes: Vec<Change> = db
        .list_changes(None, None)
        .unwrap_or_default()
        .into_iter()
        .filter(|c| target.matches(c) && cutoff.as_deref().is_none_or(|t| c.time.start.as_str() >= t))
        .collect();
    changes.sort_by(|a, b| a.time.start.cmp(&b.time.start));
    if changes.is_empty() {
        eprintln!("trail: nothing matches (give --session, --intent or --actor)");
        process::exit(1);
    }
    if json {
        let rows: Vec<_> = changes
            .iter()
            .map(|c| {
                let mut v = serde_json::to_value(c).unwrap_or_default();
                v["why"] = serde_json::json!(reason_text(Some(&db), c));
                v
            })
            .collect();
        println!("{}", serde_json::json!({ "changes": rows }));
        return;
    }
    print!("{}", markdown(&db, &changes));
}

fn markdown(db: &Database, changes: &[Change]) -> String {
    let mut files: Vec<&str> = changes.iter().map(|c| c.path.as_str()).collect();
    files.sort();
    files.dedup();
    let mut out = String::from("# Trail report\n\n");
    out.push_str(&format!(
        "{} change{} in {} file{}, {} to {}.\n\n",
        changes.len(),
        if changes.len() == 1 { "" } else { "s" },
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        changes[0].time.start,
        changes[changes.len() - 1].time.start
    ));
    out.push_str("## Files\n\n");
    for f in &files {
        let of: Vec<_> = changes.iter().filter(|c| c.path == *f).collect();
        let reasons: Vec<String> =
            of.iter().filter_map(|c| reason_text(Some(db), c)).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
        let why = if reasons.is_empty() { "no reason recorded".to_string() } else { reasons.join("; ") };
        out.push_str(&format!("- `{f}`: {} change{}, {why}\n", of.len(), if of.len() == 1 { "" } else { "s" }));
    }
    out.push_str("\n## Changes\n\n| time | op | file | by | id |\n|---|---|---|---|---|\n");
    for c in changes {
        out.push_str(&format!("| {} | {} | `{}` | {} | `{}` |\n", c.time.start, c.op, c.path, actor_label(&c.actor, &c.confidence), c.id));
    }
    out
}
