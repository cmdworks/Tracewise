//! Bulk changes: a burst of file-event-only changes (formatter, generator, lockfile, multi-file replace) becomes one command.
//!
//! After the daemon records a batch it saw only through file events, `BULK_MIN` or more changes nobody explained are linked to one
//! `bulk` command, so the Timeline shows one row and `tracewise undo --command <id>` reverts them together. Attribution is not changed.
use tracewise_core::db::Database;
use tracewise_core::model::{Actor, CommandRecord};

/// At least this many changes in one batch count as a bulk change.
pub(crate) const BULK_MIN: usize = 8;

pub(crate) fn text(n: usize) -> String {
    format!("{n} files changed at once (a formatter, generator, lockfile or multi-file replace?)")
}

/// Records the command and links the changes; returns its id. Fails open.
pub(crate) fn note(db: &Database, ids: &[String]) -> Option<String> {
    let free: Vec<String> = ids
        .iter()
        .filter(|id| db.get_change(id).ok().flatten().is_some_and(|c| c.source == "watch" && c.command.is_none()))
        .cloned()
        .collect();
    if free.len() < BULK_MIN {
        return None;
    }
    let now = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let rec = CommandRecord {
        id: ulid::Ulid::new().to_string(),
        kind: "bulk".to_string(),
        text: text(free.len()),
        cwd: None,
        actor: Actor { kind: "unknown".to_string(), name: None, session: None },
        start: now.clone(),
        end: now,
        exit_code: 0,
        changes: free.clone(),
    };
    db.insert_command(&rec).ok()?;
    db.link_changes_to_command(&free, &rec.id).ok()?;
    Some(rec.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tracewise_core::model::{Change, GitInfo, TimeRange};

    fn put(db: &Database, source: &str) -> String {
        let id = ulid::Ulid::new().to_string();
        let t = tracewise_core::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let c = Change {
            id: id.clone(),
            time: TimeRange { start: t.clone(), end: t },
            path: format!("f{id}"),
            op: "modify".into(),
            old_path: None,
            hunks: vec![],
            before: None,
            after: None,
            actor: Actor { kind: "unknown".into(), name: None, session: None },
            candidates: vec![],
            confidence: "unknown".into(),
            source: source.into(),
            command: None,
            intent: None,
            lock_conflict: None,
            device: "d".into(),
            git: GitInfo { branch: "m".into(), head: "h".into() },
            prev_hash: None,
            undoes: None,
        };
        db.insert_change(&c).unwrap();
        id
    }

    #[test]
    fn a_burst_becomes_one_command_and_a_small_batch_does_not() {
        let tmp = tempdir().unwrap();
        let db = Database::open(&tmp.path().join("tracewise.db")).unwrap();
        let few: Vec<String> = (0..BULK_MIN - 1).map(|_| put(&db, "watch")).collect();
        assert!(note(&db, &few).is_none());
        let mut many = few.clone();
        many.push(put(&db, "watch"));
        many.push(put(&db, "hook")); // not file-event-only: left alone
        let cid = note(&db, &many).unwrap();
        assert_eq!(db.get_change(&many[0]).unwrap().unwrap().command.as_deref(), Some(cid.as_str()));
        assert!(db.get_change(many.last().unwrap()).unwrap().unwrap().command.is_none());
        assert!(note(&db, &many).is_none(), "already linked");
    }
}
