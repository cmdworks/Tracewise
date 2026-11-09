//! Applying line updates and change records to the database.
use crate::db::Database;
use crate::linemap::LineMap;
use crate::model::{Change, GitInfo, Hunk, TimeRange};
use std::io;

/// What a scan stamps on every new change record. Both scans build records here, so they cannot drift apart.
pub(crate) struct Stamp<'a> {
    pub options: &'a super::ScanOptions,
    pub branch: &'a str,
    pub head: &'a str,
    pub device: &'a str,
}

impl Stamp<'_> {
    pub(crate) fn change(
        &self,
        db: &Database,
        path: &str,
        default_op: &str,
        is_modify: bool,
        (hunks, before, after): (Vec<Hunk>, Option<String>, Option<String>),
    ) -> Change {
        let (actor, candidates, confidence, source_hint) =
            super::actor::resolve_actor_candidates_confidence(db, self.options, path, is_modify);
        let undoes = self.options.path_undoes.as_ref().and_then(|m| m.get(path)).cloned().or_else(|| self.options.undoes.clone());
        Change {
            id: ulid::Ulid::new().to_string(),
            time: TimeRange::now(),
            path: path.to_string(),
            op: self.options.op.clone().unwrap_or_else(|| default_op.to_string()),
            old_path: None,
            hunks,
            before,
            after,
            lock_conflict: super::actor::check_lock_conflict(db, path, &actor),
            actor,
            candidates,
            confidence,
            source: source_hint.unwrap_or_else(|| self.options.source.clone()),
            command: None,
            intent: None,
            device: self.device.to_string(),
            git: GitInfo { branch: self.branch.to_string(), head: self.head.to_string() },
            prev_hash: None,
            undoes,
        }
    }
}

/// The lines of a text file (not binary, not holding a secret: those have no line map); Latin-1 and other non-UTF-8 text is read lossily (lines are what blame needs), binary is not text.
fn text_of(bytes: &[u8]) -> Option<std::borrow::Cow<'_, str>> {
    (!bytes.contains(&0) && !crate::blob::holds_secret(bytes)).then(|| String::from_utf8_lossy(bytes))
}

pub(crate) fn record_modify(
    db: &mut Database,
    result_ids: &mut Vec<String>,
    m: &Change,
    before_bytes: Option<&[u8]>,
    after_bytes: &[u8],
) -> io::Result<()> {
    result_ids.push(m.id.clone());
    let before_text = before_bytes.and_then(text_of);
    let after_text = text_of(after_bytes);
    let (before_str, after_str) = (before_text.as_deref(), after_text.as_deref());

    let old_map = db.get_file_line_map(&m.path).map_err(|e| io::Error::other(e.to_string()))?.unwrap_or_default();
    let existing_changes = db.get_file_changes_map(&m.path).map_err(|e| io::Error::other(e.to_string()))?;
    let next_seq = existing_changes.keys().copied().max().unwrap_or(0) + 1;

    let new_map = old_map.apply_diff(before_str, after_str, next_seq);
    db.set_file_line_map(&m.path, &new_map, &[(next_seq, m.id.as_str())]).map_err(|e| io::Error::other(e.to_string()))?;

    db.insert_change(m).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

pub(crate) fn record_create(db: &mut Database, result_ids: &mut Vec<String>, change: &Change, bytes: &[u8]) -> io::Result<()> {
    result_ids.push(change.id.clone());
    if let Some(text) = text_of(bytes) {
        let line_count = text.lines().count();
        if line_count > 0 {
            let mut map = LineMap::new();
            map.push_run(1, line_count as u32);
            db.set_file_line_map(&change.path, &map, &[(1, change.id.as_str())]).map_err(|e| io::Error::other(e.to_string()))?;
        }
    }

    db.insert_change(change).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

pub(crate) fn record_delete(db: &Database, result_ids: &mut Vec<String>, change: &Change) -> io::Result<()> {
    result_ids.push(change.id.clone());
    db.insert_change(change).map_err(|e| io::Error::other(e.to_string()))?;
    db.delete_file_lines(&change.path).map_err(|e| io::Error::other(e.to_string()))?;
    db.delete_file(&change.path).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}

pub(crate) fn record_rename(db: &Database, result_ids: &mut Vec<String>, change: &Change, del_path: &str) -> io::Result<()> {
    result_ids.push(change.id.clone());
    db.insert_change(change).map_err(|e| io::Error::other(e.to_string()))?;
    db.delete_file(del_path).map_err(|e| io::Error::other(e.to_string()))?;
    db.rename_file_lines(del_path, &change.path).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(())
}
