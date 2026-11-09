//! `trail prune`: removes records from the history and leaves the proof that keeps `verify` passing.
//! A run of removed records between two survivors becomes one `chain_meta` row `gap:<id of the survivor before>` holding the
//! hash of the last removed record, which is what the next survivor's `prev_hash` points at. Each prune also leaves a
//! `prune:<ulid>` row saying what was removed, by whom and from which folders.
use super::Database;
use rusqlite::{params, Result};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct PruneReport {
    /// Records removed (or that would be, in a dry run)
    pub removed: usize,
    /// Runs of consecutive removed records, each bridged by one `gap:` row
    pub gaps: usize,
    /// Removed records per top folder (a file at the root counts under its own name)
    pub folders: BTreeMap<String, usize>,
}

impl Database {
    /// The hash a survivor's `prev_hash` may point at after the run of records that followed `after_id` (empty = the start) was pruned.
    pub(crate) fn gap_hash(&self, after_id: &str) -> Option<String> {
        self.conn.query_row("SELECT value FROM chain_meta WHERE key = ?1", [format!("gap:{after_id}")], |r| r.get(0)).ok()
    }

    /// Ids of the records already pushed to the remote.
    pub fn pushed_change_ids(&self) -> Result<HashSet<String>> {
        let mut stmt = self.conn.prepare("SELECT id FROM changes WHERE pushed = 1")?;
        let ids = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<HashSet<_>>>()?;
        Ok(ids)
    }

    /// Removes the records in `ids` in one transaction. Never removed: the newest record (the chain head) and anything at or
    /// before the legacy boundary. With `forget_state`, a path left with no record also loses its `files`, `file_modes`,
    /// `line_map` and `file_changes` rows (right for ignored paths; for a path still on disk it would be recorded again).
    /// `dry_run` only counts. `actor` and `selector` go into the audit row.
    pub fn prune_changes(
        &self,
        ids: &HashSet<String>,
        actor: &str,
        selector: &str,
        forget_state: bool,
        dry_run: bool,
    ) -> Result<PruneReport> {
        self.with_write_lock(|| {
            let changes = self.all_changes_in_insert_order()?;
            let legacy_end = self.legacy_boundary().and_then(|id| changes.iter().position(|c| c.id == id)).map(|i| i + 1).unwrap_or(0);
            let newest = changes.len().saturating_sub(1);
            let remove: Vec<bool> =
                changes.iter().enumerate().map(|(i, c)| i >= legacy_end && i != newest && ids.contains(&c.id)).collect();

            let mut report = PruneReport::default();
            let mut gaps: Vec<(String, String)> = Vec::new(); // (id of the survivor before the run, hash the next survivor points at)
            let mut i = 0;
            while i < changes.len() {
                if !remove[i] {
                    i += 1;
                    continue;
                }
                let mut j = i;
                while j + 1 < changes.len() && remove[j + 1] {
                    j += 1;
                }
                for c in &changes[i..=j] {
                    let top = c.path.split('/').next().unwrap_or(&c.path).to_string();
                    *report.folders.entry(top).or_default() += 1;
                }
                report.removed += j - i + 1;
                // a gap already behind the last removed record is part of this run now
                let last = &changes[j];
                let last_hash = self.gap_hash(&last.id).unwrap_or_else(|| Self::compute_change_hash(last));
                let after = if i == 0 { String::new() } else { changes[i - 1].id.clone() };
                gaps.push((after, last_hash));
                i = j + 1;
            }
            report.gaps = gaps.len();
            if dry_run || report.removed == 0 {
                return Ok(report);
            }

            let removed_ids: Vec<&str> = changes.iter().zip(&remove).filter(|(_, r)| **r).map(|(c, _)| c.id.as_str()).collect();
            let conn = &self.conn;
            {
                let mut drop_gap = conn.prepare("DELETE FROM chain_meta WHERE key = ?1")?;
                let mut del_change = conn.prepare("DELETE FROM changes WHERE id = ?1")?;
                let mut del_undo = conn.prepare("DELETE FROM undo_records WHERE id = ?1 OR undoes = ?1")?;
                let mut del_seq = conn.prepare("DELETE FROM file_changes WHERE change_id = ?1")?;
                for id in &removed_ids {
                    drop_gap.execute([format!("gap:{id}")])?;
                    del_change.execute([id])?;
                    del_undo.execute([id])?;
                    if forget_state {
                        del_seq.execute([id])?;
                    }
                }
            }
            for (after, hash) in &gaps {
                conn.execute("INSERT OR REPLACE INTO chain_meta (key, value) VALUES (?1, ?2)", params![format!("gap:{after}"), hash])?;
            }

            // commands keep a JSON list of the change ids they made
            let removed_set: HashSet<&str> = removed_ids.iter().copied().collect();
            let rows: Vec<(String, String)> = {
                let mut stmt = conn.prepare("SELECT id, changes FROM commands WHERE changes != '[]'")?;
                let found = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>>>()?;
                found
            };
            for (id, json) in rows {
                let Ok(list) = serde_json::from_str::<Vec<String>>(&json) else { continue };
                let kept: Vec<&String> = list.iter().filter(|c| !removed_set.contains(c.as_str())).collect();
                if kept.len() != list.len() {
                    conn.execute(
                        "UPDATE commands SET changes = ?1 WHERE id = ?2",
                        params![serde_json::to_string(&kept).unwrap_or_default(), id],
                    )?;
                }
            }

            if forget_state {
                let paths: HashSet<&str> = changes.iter().zip(&remove).filter(|(_, r)| **r).map(|(c, _)| c.path.as_str()).collect();
                for p in paths {
                    let left: i64 = conn.query_row("SELECT COUNT(*) FROM changes WHERE path = ?1", [p], |r| r.get(0))?;
                    if left == 0 {
                        for sql in [
                            "DELETE FROM files WHERE path = ?1",
                            "DELETE FROM file_modes WHERE path = ?1",
                            "DELETE FROM line_map WHERE path = ?1",
                            "DELETE FROM file_changes WHERE path = ?1",
                        ] {
                            conn.execute(sql, [p])?;
                        }
                    }
                }
            }

            let audit = serde_json::json!({
                "time": crate::clock::now_text(),
                "actor": actor,
                "selector": selector,
                "removed": report.removed,
                "gaps": report.gaps,
                "folders": report.folders,
            });
            conn.execute(
                "INSERT INTO chain_meta (key, value) VALUES (?1, ?2)",
                params![format!("prune:{}", ulid::Ulid::new()), audit.to_string()],
            )?;
            Ok(report)
        })
    }

    /// (id, path, start) of every record, for choosing what to prune without loading the diffs.
    pub fn change_keys(&self) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, path, start FROM changes ORDER BY rowid")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Writes a copy of the database to `file` (which must not exist), for `trail prune --backup`.
    pub fn backup_to(&self, file: &std::path::Path) -> Result<()> {
        self.conn.execute("VACUUM INTO ?1", [file.to_string_lossy().as_ref()])?;
        Ok(())
    }

    /// Hands the freed pages back to the file system after a prune.
    pub fn compact(&self) -> Result<()> {
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM; PRAGMA wal_checkpoint(TRUNCATE);")
    }

    /// The `prune:` audit rows, oldest first, as JSON text.
    pub fn prune_events(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM chain_meta WHERE key LIKE 'prune:%' ORDER BY key")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?.collect::<Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
#[path = "test_prune.rs"]
mod tests;
