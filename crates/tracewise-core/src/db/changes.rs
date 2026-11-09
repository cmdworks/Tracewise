use super::Database;
use crate::model::{glob_match, Actor, Change, GitInfo, Hunk, PendingItem, TimeRange};
use rusqlite::{params, Result};

const CHANGE_SELECT: &str = "SELECT c.id, c.start, c.end, c.path, c.op, c.old_path, c.hunks, c.before, c.after,
                    c.actor_kind, c.actor_name, c.actor_session, c.candidates, c.confidence,
                    c.source, c.command, c.intent, c.lock_conflict, c.device, c.branch, c.head, c.prev_hash,
                    u.undoes
             FROM changes c
             LEFT JOIN undo_records u ON c.id = u.id";

impl Database {
    pub fn insert_change(&self, change: &Change) -> Result<()> {
        self.with_write_lock(|| self.insert_change_unlocked(change))?;
        self.write_chain_head(change);
        Ok(())
    }

    /// Best effort: a failure to write the head file must never lose a record.
    fn write_chain_head(&self, change: &Change) {
        let body = format!("{}\n{}\n", change.id, Self::compute_change_hash(change));
        if self.conn.is_autocommit() {
            self.write_head_file(&body);
        } else {
            *self.pending_head.borrow_mut() = Some(body);
        }
    }

    pub(crate) fn write_head_file(&self, body: &str) {
        let Some(path) = &self.head_path else { return };
        let tmp = path.with_extension("tmp");
        if tracewise_platform::private::write(&tmp, body.as_bytes()).is_ok() {
            let _ = crate::blob::sync_if_full(&tmp);
            let _ = std::fs::rename(&tmp, path);
        }
    }

    fn insert_change_unlocked(&self, change: &Change) -> Result<()> {
        let hunks_json = serde_json::to_string(&change.hunks).unwrap_or_else(|_| "[]".to_string());
        let candidates_json = serde_json::to_string(&change.candidates).unwrap_or_else(|_| "[]".to_string());
        let lock_conflict_json = change.lock_conflict.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());

        // the chain is this database's own: a hash carried by a record from elsewhere is ignored
        let prev_hash = self.get_latest_change_hash()?;

        self.conn.execute(
            "INSERT INTO changes (
                id, start, end, path, op, old_path, hunks, before, after,
                actor_kind, actor_name, actor_session, candidates, confidence,
                source, command, intent, lock_conflict, device, branch, head, prev_hash
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22
            )",
            params![
                change.id,
                change.time.start,
                change.time.end,
                change.path,
                change.op,
                change.old_path,
                hunks_json,
                change.before,
                change.after,
                change.actor.kind,
                change.actor.name,
                change.actor.session,
                candidates_json,
                change.confidence,
                change.source,
                change.command,
                change.intent,
                lock_conflict_json,
                change.device,
                change.git.branch,
                change.git.head,
                prev_hash,
            ],
        )?;

        if let Some(ref undoes) = change.undoes {
            let undoes_json = serde_json::to_string(undoes).unwrap_or_else(|_| "[]".to_string());
            self.conn.execute(
                "INSERT INTO undo_records (id, undoes) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET undoes = ?2",
                params![change.id, undoes_json],
            )?;
        }

        Ok(())
    }

    pub fn insert_change_imported(&self, change: &Change) -> Result<bool> {
        let inserted = self.with_write_lock(|| self.insert_change_imported_unlocked(change))?;
        if inserted {
            self.write_chain_head(change);
        }
        Ok(inserted)
    }

    fn insert_change_imported_unlocked(&self, change: &Change) -> Result<bool> {
        let mut check_stmt = self.conn.prepare("SELECT 1 FROM changes WHERE id = ?1")?;
        let mut rows = check_stmt.query(params![change.id])?;
        if rows.next()?.is_some() {
            return Ok(false);
        }

        let hunks_json = serde_json::to_string(&change.hunks).unwrap_or_else(|_| "[]".to_string());
        let candidates_json = serde_json::to_string(&change.candidates).unwrap_or_else(|_| "[]".to_string());
        let lock_conflict_json = change.lock_conflict.as_ref().map(|v| serde_json::to_string(v).unwrap_or_default());

        // the chain is this database's own: a hash carried by a record from elsewhere is ignored
        let prev_hash = self.get_latest_change_hash()?;

        self.conn.execute(
            "INSERT INTO changes (
                id, start, end, path, op, old_path, hunks, before, after,
                actor_kind, actor_name, actor_session, candidates, confidence,
                source, command, intent, lock_conflict, device, branch, head, pushed, prev_hash
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20, ?21, 1, ?22
            )",
            params![
                change.id,
                change.time.start,
                change.time.end,
                change.path,
                change.op,
                change.old_path,
                hunks_json,
                change.before,
                change.after,
                change.actor.kind,
                change.actor.name,
                change.actor.session,
                candidates_json,
                change.confidence,
                change.source,
                change.command,
                change.intent,
                lock_conflict_json,
                change.device,
                change.git.branch,
                change.git.head,
                prev_hash,
            ],
        )?;

        if let Some(ref undoes) = change.undoes {
            let undoes_json = serde_json::to_string(undoes).unwrap_or_else(|_| "[]".to_string());
            self.conn.execute(
                "INSERT INTO undo_records (id, undoes) VALUES (?1, ?2)
                 ON CONFLICT(id) DO UPDATE SET undoes = ?2",
                params![change.id, undoes_json],
            )?;
        }

        Ok(true)
    }

    pub fn get_unpushed_changes(&self) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE pushed = 0
             ORDER BY start ASC",
        )?;
        let rows = stmt.query_map([], Self::row_to_change)?;
        let mut changes = Vec::new();
        for r in rows {
            changes.push(r?);
        }
        Ok(changes)
    }

    pub fn mark_changes_pushed(&self, ids: &[String]) -> Result<()> {
        let mut stmt = self.conn.prepare("UPDATE changes SET pushed = 1 WHERE id = ?1")?;
        for id in ids {
            stmt.execute(params![id])?;
        }
        Ok(())
    }

    /// Marks every record made on `device` as not pushed, so the next push sends them all again (`trail share key rekey`).
    pub fn mark_device_unpushed(&self, device: &str) -> Result<usize> {
        self.conn.execute("UPDATE changes SET pushed = 0 WHERE device = ?1", params![device])
    }

    pub fn get_change(&self, id: &str) -> Result<Option<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.start, c.end, c.path, c.op, c.old_path, c.hunks, c.before, c.after,
                    c.actor_kind, c.actor_name, c.actor_session, c.candidates, c.confidence,
                    c.source, c.command, c.intent, c.lock_conflict, c.device, c.branch, c.head, c.prev_hash,
                    u.undoes
             FROM changes c
             LEFT JOIN undo_records u ON c.id = u.id
             WHERE c.id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_change(row)?))
        } else {
            Ok(None)
        }
    }

    /// Every change that started at or after `since`, newest first (no limit: a bulk edit can touch hundreds of files).
    pub fn list_changes_since(&self, since: &str) -> Result<Vec<Change>> {
        let sql = format!("{CHANGE_SELECT} WHERE c.start >= ?1 ORDER BY c.id DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![since], Self::row_to_change)?;
        rows.collect()
    }

    pub fn list_changes(&self, path_filter: Option<&str>, limit: Option<usize>) -> Result<Vec<Change>> {
        let base_sql = CHANGE_SELECT;

        let limit_val = limit.map(|l| l as i64).unwrap_or(-1);

        let mut results = Vec::new();
        if let Some(path) = path_filter {
            let sql = format!("{base_sql} WHERE c.path = ?1 ORDER BY c.id DESC LIMIT ?2");
            let mut stmt = self.conn.prepare(&sql)?;
            let mut rows = stmt.query(params![path, limit_val])?;
            while let Some(row) = rows.next()? {
                results.push(Self::row_to_change(row)?);
            }

            // If the path was not created by a logged change, check if it was part of a baseline snapshot. Only when the whole
            // history fit under the limit: otherwise an older create may exist, and the baseline query scans every change.
            let all_listed = limit_val < 0 || (results.len() as i64) < limit_val;
            if all_listed && !results.iter().any(|c| c.op == "create") && path != "." {
                if let Ok(Some(file_rec)) = self.get_file(path) {
                    let bsql = format!("{base_sql} WHERE c.op = 'baseline' ORDER BY c.start ASC LIMIT 1");
                    if let Ok(mut bstmt) = self.conn.prepare(&bsql) {
                        if let Ok(mut brows) = bstmt.query([]) {
                            if let Ok(Some(brow)) = brows.next() {
                                if let Ok(mut bch) = Self::row_to_change(brow) {
                                    bch.path = path.to_string();
                                    bch.after = Some(file_rec.blob);
                                    results.push(bch);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            let sql = format!("{base_sql} ORDER BY c.id DESC LIMIT ?1");
            let mut stmt = self.conn.prepare(&sql)?;
            let mut rows = stmt.query(params![limit_val])?;
            while let Some(row) = rows.next()? {
                results.push(Self::row_to_change(row)?);
            }
        }
        Ok(results)
    }

    pub fn list_pending_changes(&self) -> Result<Vec<PendingItem>> {
        let sql = "SELECT id, start, end, path, actor_kind, actor_name, actor_session
                   FROM changes
                   WHERE intent IS NULL AND source != 'baseline'
                   ORDER BY start DESC, id DESC";
        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let start: String = row.get(1)?;
            let end: String = row.get(2)?;
            let path: String = row.get(3)?;
            let kind: String = row.get(4)?;
            let name: Option<String> = row.get(5)?;
            let session: Option<String> = row.get(6)?;
            Ok(PendingItem { id, path, actor: Actor { kind, name, session }, time: TimeRange { start, end } })
        })?;
        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }
        Ok(items)
    }

    pub fn get_shell_pending_count(&self, shell_id: &str) -> Result<Option<usize>> {
        let mut stmt = self.conn.prepare("SELECT pending_count FROM shell_state WHERE shell_id = ?1")?;
        let mut rows = stmt.query(params![shell_id])?;
        if let Some(row) = rows.next()? {
            let count: i64 = row.get(0)?;
            Ok(Some(count as usize))
        } else {
            Ok(None)
        }
    }

    pub fn set_shell_pending_count(&self, shell_id: &str, count: usize) -> Result<()> {
        self.conn.execute(
            "INSERT INTO shell_state (shell_id, pending_count) VALUES (?1, ?2)
             ON CONFLICT(shell_id) DO UPDATE SET pending_count = ?2",
            params![shell_id, count as i64],
        )?;
        Ok(())
    }

    pub(crate) fn row_to_change(row: &rusqlite::Row) -> rusqlite::Result<Change> {
        let hunks_str: String = row.get(6)?;
        let hunks: Vec<Hunk> = serde_json::from_str(&hunks_str).unwrap_or_default();
        let candidates_str: String = row.get(12)?;
        let candidates: Vec<Actor> = serde_json::from_str(&candidates_str).unwrap_or_default();
        let lock_conflict_str: Option<String> = row.get(17)?;
        let lock_conflict: Option<serde_json::Value> = lock_conflict_str.and_then(|s| serde_json::from_str(&s).ok());

        Ok(Change {
            id: row.get(0)?,
            time: TimeRange { start: row.get(1)?, end: row.get(2)? },
            path: row.get(3)?,
            op: row.get(4)?,
            old_path: row.get(5)?,
            hunks,
            before: row.get(7)?,
            after: row.get(8)?,
            actor: Actor { kind: row.get(9)?, name: row.get(10)?, session: row.get(11)? },
            candidates,
            confidence: row.get(13)?,
            source: row.get(14)?,
            command: row.get(15)?,
            intent: row.get(16)?,
            lock_conflict,
            device: row.get(18)?,
            git: GitInfo { branch: row.get(19)?, head: row.get(20)? },
            prev_hash: row.get(21).ok(),
            undoes: row.get::<_, Option<String>>(22).ok().flatten().and_then(|s| serde_json::from_str(&s).ok()),
        })
    }

    pub fn count_changes(&self) -> Result<usize> {
        let count: usize = self.conn.query_row("SELECT count(*) FROM changes", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Changes on `path` that landed in the `secs` seconds before `before` (RFC 3339), newest first, baseline excluded.
    fn changes_just_before(&self, path: &str, before: &str, secs: i64) -> Result<Vec<Change>> {
        let Ok(end) = chrono::DateTime::parse_from_rfc3339(before) else {
            return Ok(Vec::new());
        };
        let from = (end - chrono::Duration::seconds(secs)).with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE path = ?1 AND start < ?2 AND start >= ?3 AND actor_kind != 'baseline'
             ORDER BY start DESC LIMIT 10",
        )?;
        let rows = stmt.query_map(params![path, before, from], Self::row_to_change)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
    }

    /// The edit by someone else that `change` landed right on top of (within `secs` seconds), if any: the overlap two
    /// editors cause without any lock being involved.
    pub fn overlapping_prior(&self, change: &Change, secs: i64) -> Result<Option<Change>> {
        // Only when both writers are known: an `unknown` change is often the same person before the shell hook named them.
        let known = |a: &Actor| a.kind == "human" || a.kind == "agent";
        if !known(&change.actor) {
            return Ok(None);
        }
        Ok(self
            .changes_just_before(&change.path, &change.time.start, secs)?
            .into_iter()
            .find(|c| known(&c.actor) && !c.actor.is_same(&change.actor)))
    }

    /// A person (or an editor with no hook: actor unknown) changed `path` in the last `secs` seconds: the latest such
    /// change. An agent about to edit the file waits for it.
    pub fn recent_person_edit(&self, path: &str, secs: i64) -> Result<Option<Change>> {
        let now = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        Ok(self.changes_just_before(path, &now, secs)?.into_iter().find(|c| c.actor.kind == "human" || c.actor.kind == "unknown"))
    }

    pub fn upgrade_change_actor(&self, change_id: &str, actor: &Actor, confidence: &str, source: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE changes SET actor_kind = ?1, actor_name = ?2, actor_session = ?3, confidence = ?4, source = ?5 WHERE id = ?6",
            params![actor.kind, actor.name, actor.session, confidence, source, change_id,],
        )?;
        Ok(())
    }

    pub fn find_unknown_changes_for_path_since(&self, path: &str, since_time: &str) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE path = ?1 AND start >= ?2 AND actor_kind = 'unknown'
             ORDER BY start DESC",
        )?;
        let rows = stmt.query_map(params![path, since_time], Self::row_to_change)?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn find_unknown_changes_since(&self, since_time: &str) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE start >= ?1 AND actor_kind = 'unknown'
             ORDER BY start DESC",
        )?;
        let rows = stmt.query_map(params![since_time], Self::row_to_change)?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Changes the scan could not attribute because several actors were active, newest first.
    pub fn find_ambiguous_changes_since(&self, since_time: &str) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE start >= ?1 AND actor_kind = 'ambiguous'
             ORDER BY start DESC",
        )?;
        let rows = stmt.query_map(params![since_time], Self::row_to_change)?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Settles the ambiguous changes since `since` that belong to `actor`: it is one of the candidates, a reason it
    /// declared covers the file, and every other candidate is an agent that did not declare it. Returns their ids.
    pub fn settle_ambiguous_for(&self, actor: &Actor, since: &str) -> Result<Vec<String>> {
        let (Some(name), session) = (actor.name.as_deref(), actor.session.as_deref()) else {
            return Ok(Vec::new());
        };
        let mut settled = Vec::new();
        for c in self.find_ambiguous_changes_since(since)? {
            if !c.candidates.iter().any(|a| a.is_same(actor)) {
                continue;
            }
            if self.find_covering_intent(name, session, &c.path)?.is_none() {
                continue;
            }
            // A person among the candidates cannot be ruled out, so only agents that never declared the file can be.
            let rival = c.candidates.iter().any(|a| {
                !a.is_same(actor)
                    && (a.kind != "agent"
                        || a.name
                            .as_deref()
                            .is_some_and(|n| self.find_covering_intent(n, a.session.as_deref(), &c.path).ok().flatten().is_some()))
            });
            if !rival {
                self.upgrade_change_actor(&c.id, actor, "likely", "hook")?;
                settled.push(c.id);
            }
        }
        Ok(settled)
    }

    pub fn get_agent_changes_for_branch(&self, branch: &str) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE branch = ?1 AND actor_kind = 'agent'
             ORDER BY start ASC",
        )?;
        let rows = stmt.query_map(params![branch], Self::row_to_change)?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_agent_changes_for_session(&self, agent: &str, session: &str) -> Result<Vec<Change>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE actor_name = ?1 AND actor_session = ?2 AND actor_kind = 'agent'
             ORDER BY start ASC",
        )?;
        let rows = stmt.query_map(params![agent, session], Self::row_to_change)?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn set_change_lock_conflict(&self, change_id: &str, conflict: &serde_json::Value) -> Result<()> {
        let conflict_json = serde_json::to_string(conflict).unwrap_or_default();
        self.conn.execute("UPDATE changes SET lock_conflict = ?1 WHERE id = ?2", params![conflict_json, change_id])?;
        Ok(())
    }

    /// The newest `limit` changes of one file made since `since` by anyone but `session` (people and unnamed writers included).
    /// Uses the `(path, start)` index, so the cost does not grow with the size of the history.
    /// When this actor name last changed anything here: the baseline of the first-prompt digest.
    pub fn last_start_by_actor(&self, name: &str) -> Result<Option<String>> {
        self.conn.query_row("SELECT MAX(start) FROM changes WHERE actor_name = ?1", params![name], |r| r.get(0))
    }

    /// `(path, who, confidence)` of the newest changes after `since` (newest first, at most `limit`). Called with the time this
    /// actor last changed anything, so none of them is its own.
    pub fn changes_after(&self, since: &str, limit: usize) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT path, COALESCE(actor_name, actor_kind), confidence FROM changes WHERE start > ?1 ORDER BY start DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![since, limit as i64], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect()
    }

    pub fn changes_by_others_since(&self, path: &str, since: &str, session: &str, limit: usize) -> Result<Vec<Change>> {
        let sql = format!(
            "{CHANGE_SELECT} WHERE c.path = ?1 AND c.start >= ?2 AND (c.actor_session IS NULL OR c.actor_session != ?3)
             ORDER BY c.start DESC LIMIT ?4"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![path, since, session, limit as i64], Self::row_to_change)?;
        rows.collect()
    }

    pub fn get_changes_since_for_paths(&self, since_time: &str, session_to_exclude: &str, paths: &[String]) -> Result<Vec<Change>> {
        if paths.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = self.conn.prepare(
            "SELECT id, start, end, path, op, old_path, hunks, before, after,
                    actor_kind, actor_name, actor_session, candidates, confidence,
                    source, command, intent, lock_conflict, device, branch, head, prev_hash
             FROM changes
             WHERE start >= ?1
             ORDER BY start ASC, id ASC",
        )?;
        let rows = stmt.query_map(params![since_time], Self::row_to_change)?;

        let mut result = Vec::new();
        for change_res in rows {
            let change = change_res?;
            if change.actor.session.as_deref() == Some(session_to_exclude) {
                continue;
            }
            if paths.iter().any(|p| p == &change.path || glob_match(p, &change.path)) {
                result.push(change);
            }
        }

        Ok(result)
    }
}

#[cfg(test)]
#[path = "test_changes.rs"]
mod tests;
