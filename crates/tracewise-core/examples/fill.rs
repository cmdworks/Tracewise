//! Test data generator (not part of the `tracewise` program): fills an initialised repository's history with many changes and
//! commands, with a valid hash chain, so scale and migration tests need no hours of scanning.
//!   cargo run --release -p tracewise-core --example fill -- <repo> <changes> [commands] [files]
use std::path::PathBuf;
use tracewise_core::db::Database;
use tracewise_core::model::{Actor, Change, CommandRecord, GitInfo, TimeRange};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let repo = PathBuf::from(args.first().expect("usage: fill <repo> <changes> [commands] [files]"));
    let num = |i: usize, d: usize| args.get(i).and_then(|v| v.parse().ok()).unwrap_or(d);
    let (changes, commands, files) = (num(1, 1000), num(2, 0), num(3, 500).max(1));
    let db = Database::open(&repo.join(".git/tracewise/tracewise.db")).expect("open: run `tracewise init` first");
    let actors = [
        Actor::agent("claude", "fill1"),
        Actor::agent("codex", "fill2"),
        Actor { kind: "human".into(), name: Some("you".into()), session: None },
    ];
    let start = chrono::Utc::now() - chrono::Duration::seconds((changes + commands) as i64);
    db.begin_batch().unwrap();
    for i in 0..changes {
        let t = (start + chrono::Duration::seconds(i as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let actor = actors[i % actors.len()].clone();
        let ch = Change {
            id: ulid::Ulid::new().to_string(),
            time: TimeRange { start: t.clone(), end: t },
            path: format!("dir{}/file{}.txt", (i % files) / 50, i % files),
            op: "modify".into(),
            old_path: None,
            hunks: Vec::new(),
            before: None,
            after: None,
            candidates: vec![actor.clone()],
            actor,
            confidence: "exact".into(),
            source: "fill".into(),
            command: None,
            intent: None,
            lock_conflict: None,
            device: "fill".into(),
            git: GitInfo { branch: "main".into(), head: "0".repeat(40) },
            prev_hash: None,
            undoes: None,
        };
        db.insert_change(&ch).unwrap();
        if i % 50_000 == 49_999 {
            db.commit_batch().unwrap();
            db.begin_batch().unwrap();
        }
    }
    for i in 0..commands {
        let t = (start + chrono::Duration::seconds(i as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let rec = CommandRecord {
            id: ulid::Ulid::new().to_string(),
            kind: "shell".into(),
            text: format!("make target{} && ./run --iteration {i} {}", i % 97, "x".repeat(300)),
            cwd: Some(repo.display().to_string()),
            actor: actors[i % actors.len()].clone(),
            start: t.clone(),
            end: t,
            exit_code: 0,
            changes: Vec::new(),
        };
        db.insert_command(&rec).unwrap();
        if i % 50_000 == 49_999 {
            db.commit_batch().unwrap();
            db.begin_batch().unwrap();
        }
    }
    db.commit_batch().unwrap();
    println!("filled {changes} changes and {commands} commands");
}
