use super::{Database, VerifyReport};
use crate::model::{Change, FileRecord};
use rusqlite::Result;
use std::path::Path;

impl Database {
    pub(crate) fn all_changes_in_insert_order(&self) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             ORDER BY rowid ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_change)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_latest_change_hash(&self) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             ORDER BY rowid DESC
             LIMIT 1",
        )?;
        let mut rows = stmt.query_map([], Self::row_to_change)?;
        if let Some(r) = rows.next() {
            let last_change = r?;
            Ok(Some(Self::compute_change_hash(&last_change)))
        } else {
            Ok(None)
        }
    }

    /// Hash of the fields a change keeps for its whole life. Reasons, the actor (hooks upgrade it later),
    /// confidence, lock conflicts and the like are edited afterwards and are not part of the chain.
    pub fn compute_change_hash(change: &Change) -> String {
        let core = serde_json::json!([
            change.id,
            change.time.start,
            change.path,
            change.op,
            change.old_path,
            change.before,
            change.after,
            change.device,
        ]);
        blake3::hash(core.to_string().as_bytes()).to_hex().to_string()
    }

    /// A shared batch whose records are in this database is noted as `imported:<batch id>` in `chain_meta` (in the same transaction as the
    /// import), so a later `fetch` does not open it again; the note goes with the database, so a restored or rebuilt one reads every batch.
    pub fn mark_batch_imported(&self, batch_id: &str) -> Result<()> {
        self.conn.execute("INSERT OR REPLACE INTO chain_meta (key, value) VALUES (?1, '1')", [format!("imported:{batch_id}")])?;
        Ok(())
    }

    pub fn batch_imported(&self, batch_id: &str) -> bool {
        self.conn.query_row("SELECT 1 FROM chain_meta WHERE key = ?1", [format!("imported:{batch_id}")], |_| Ok(())).is_ok()
    }

    /// The id of the last change written before the hash chain existed, when this database was migrated from such a version.
    pub(crate) fn legacy_boundary(&self) -> Option<String> {
        self.conn.query_row("SELECT value FROM chain_meta WHERE key = 'legacy_upto'", [], |r| r.get(0)).ok()
    }

    /// Ids of the changes whose stored text version (`after`, when the change has line hunks) is gone from the blob store.
    /// Hash-only files (big, binary, links) never have hunks and are not expected there.
    pub fn changes_with_missing_blobs(&self, blobs: &crate::blob::BlobStore) -> Result<Vec<String>> {
        let changes = self.all_changes_in_insert_order()?;
        let mut by_blob: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
        // another device's change may come from a `summary` batch, which never carries intermediate contents: not a loss here
        let me = crate::get_device_id();
        for c in changes.iter().filter(|c| !c.hunks.is_empty() && c.device == me) {
            if let Some(after) = c.after.as_deref() {
                by_blob.entry(after).or_default().push(&c.id);
            }
        }
        let hashes: Vec<String> = by_blob.keys().map(|h| h.to_string()).collect();
        let mut ids: Vec<String> =
            blobs.missing(&hashes).iter().flat_map(|h| by_blob[h.as_str()].iter().map(|id| id.to_string())).collect();
        ids.sort();
        Ok(ids)
    }

    pub fn verify_hash_chain(&self) -> Result<VerifyReport> {
        let changes = self.all_changes_in_insert_order()?;

        if changes.is_empty() {
            return Ok(VerifyReport { ok: true, changes: 0, first_bad: None });
        }

        // History from before the chain existed (schema 6 and older) cannot be checked with today's hash; the migration
        // recorded the last such change, and the chain is verified from the next one on
        let start = self.legacy_boundary().and_then(|id| changes.iter().position(|c| c.id == id)).map(|i| i + 1).unwrap_or(0);
        let mut expected_prev_hash: Option<String> = start.checked_sub(1).map(|i| Self::compute_change_hash(&changes[i]));
        for (i, change) in changes.iter().enumerate().skip(start) {
            // after `trail prune`, a survivor may point at the last removed record; the `gap:` row for the survivor before it vouches for that
            let bridged = || {
                let after = if i == 0 { "" } else { changes[i - 1].id.as_str() };
                change.prev_hash.is_some() && self.gap_hash(after) == change.prev_hash
            };
            if i == 0 {
                if change.prev_hash.is_some() && !bridged() {
                    return Ok(VerifyReport { ok: false, changes: changes.len(), first_bad: Some(change.id.clone()) });
                }
            } else if change.prev_hash != expected_prev_hash && !bridged() {
                let bad_id = changes[i - 1].id.clone();
                return Ok(VerifyReport { ok: false, changes: changes.len(), first_bad: Some(bad_id) });
            }
            expected_prev_hash = Some(Self::compute_change_hash(change));
        }

        // The recorded head must still be in the chain with the same hash; otherwise records were removed.
        if let Some(head) = self.head_path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()) {
            let mut lines = head.lines();
            if let (Some(id), Some(hash)) = (lines.next(), lines.next()) {
                let still_there = changes.iter().any(|c| c.id == id && Self::compute_change_hash(c) == hash);
                if !still_there {
                    return Ok(VerifyReport { ok: false, changes: changes.len(), first_bad: Some(id.to_string()) });
                }
            }
        }

        Ok(VerifyReport { ok: true, changes: changes.len(), first_bad: None })
    }

    pub fn import_spool(&self, data_dir: &Path) -> Result<usize> {
        let spool_dir = data_dir.join("spool");
        if !spool_dir.is_dir() {
            return Ok(0);
        }
        let mut entries = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&spool_dir) {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) == Some("json") {
                    entries.push(p);
                }
            }
        }
        entries.sort();
        let mut imported = 0;
        for file in entries {
            if let Ok(content) = std::fs::read_to_string(&file) {
                if let Ok(mut ch) = serde_json::from_str::<Change>(&content) {
                    if ch.intent.is_none() && ch.actor.session.is_some() {
                        let agent = ch.actor.name.as_deref().unwrap_or("");
                        let session = ch.actor.session.as_deref();
                        if let Ok(Some(intent)) = self.find_any_covering_intent(agent, session, &ch.path) {
                            ch.intent = Some(intent.id);
                        }
                    }
                    if self.insert_change_imported(&ch)? {
                        if let Some(ref after_hash) = ch.after {
                            let _ = self.set_file(&FileRecord {
                                path: ch.path.clone(),
                                blob: after_hash.clone(),
                                size: 0,
                                mtime_ns: crate::clock::now().timestamp_nanos_opt().unwrap_or(0),
                            });
                        } else {
                            let _ = self.delete_file(&ch.path);
                        }
                    }
                    let _ = std::fs::remove_file(&file);
                    imported += 1;
                }
            }
        }
        Ok(imported)
    }
}

#[cfg(test)]
#[path = "test_chain.rs"]
mod tests;
