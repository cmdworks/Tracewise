use super::Database;
use crate::model::{Actor, LockRecord};
use rusqlite::{params, Result};
use std::collections::HashMap;
use std::path::Path;

#[cfg(test)]
mod tests;

impl Database {
    /// Takes or renews the lock on `path`; `Some(record)` is the live lock of someone else.
    /// Hooks are separate processes, so `BEGIN IMMEDIATE` takes SQLite's write lock before the read:
    /// without it two agents could both read "free" and both be told they hold the file.
    pub fn acquire_lock(&self, path: &str, holder: &Actor, intent_id: Option<&str>, ttl_secs: u64) -> Result<Option<LockRecord>> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = self.acquire_lock_in_tx(path, holder, intent_id, ttl_secs);
        self.conn.execute_batch(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        result
    }

    /// All-or-nothing lock acquisition for multiple paths in sorted order.
    /// If any path is locked by another session, takes none and returns the conflicting record.
    pub fn acquire_locks_atomic(
        &self,
        paths: &[String],
        holder: &Actor,
        intent_ids: &HashMap<String, Option<String>>,
        ttl_secs: u64,
    ) -> Result<Option<LockRecord>> {
        if paths.is_empty() {
            return Ok(None);
        }

        let mut sorted_paths = paths.to_vec();
        sorted_paths.sort();
        sorted_paths.dedup();

        self.conn.execute_batch("BEGIN IMMEDIATE")?;

        let now_str = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        // Check all paths first
        for p in &sorted_paths {
            let mut stmt = self.conn.prepare("SELECT path, holder, since, expires, intent FROM locks WHERE path = ?1")?;
            let mut rows = stmt.query(params![p])?;
            if let Some(row) = rows.next()? {
                let holder_str: String = row.get(1)?;
                let existing_holder: Actor =
                    serde_json::from_str(&holder_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
                let expires: String = row.get(3)?;
                if expires > now_str {
                    let same_session = match (&existing_holder.session, &holder.session) {
                        (Some(s1), Some(s2)) => s1 == s2,
                        _ => false,
                    };
                    let same_name = match (&existing_holder.name, &holder.name) {
                        (Some(n1), Some(n2)) => n1 == n2,
                        (None, None) => true,
                        _ => false,
                    };
                    if !(same_session && same_name) {
                        let conflict =
                            LockRecord { path: row.get(0)?, holder: existing_holder, since: row.get(2)?, expires, intent: row.get(4)? };
                        self.conn.execute_batch("ROLLBACK")?;
                        return Ok(Some(conflict));
                    }
                }
            }
        }

        // All paths free or owned by this session: acquire all
        for p in &sorted_paths {
            let intent_id = intent_ids.get(p).and_then(|opt| opt.as_deref());
            if let Err(e) = self.acquire_lock_in_tx(p, holder, intent_id, ttl_secs) {
                self.conn.execute_batch("ROLLBACK")?;
                return Err(e);
            }
        }

        self.conn.execute_batch("COMMIT")?;
        Ok(None)
    }

    fn acquire_lock_in_tx(&self, path: &str, holder: &Actor, intent_id: Option<&str>, ttl_secs: u64) -> Result<Option<LockRecord>> {
        let now = crate::clock::now();
        let now_str = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let new_expiry = (now + chrono::Duration::seconds(ttl_secs as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let mut stmt = self.conn.prepare("SELECT path, holder, since, expires, intent FROM locks WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let holder_str: String = row.get(1)?;
            let existing_holder: Actor =
                serde_json::from_str(&holder_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
            let existing_record = LockRecord {
                path: row.get(0)?,
                holder: existing_holder.clone(),
                since: row.get(2)?,
                expires: row.get(3)?,
                intent: row.get(4)?,
            };

            if existing_record.expires > now_str {
                let same_session = match (&existing_holder.session, &holder.session) {
                    (Some(s1), Some(s2)) => s1 == s2,
                    _ => false,
                };
                let same_name = match (&existing_holder.name, &holder.name) {
                    (Some(n1), Some(n2)) => n1 == n2,
                    (None, None) => true,
                    _ => false,
                };

                if same_session && same_name {
                    let holder_json = serde_json::to_string(holder).unwrap_or_default();
                    self.conn.execute(
                        "UPDATE locks SET expires = ?1, holder = ?2, intent = COALESCE(?3, intent) WHERE path = ?4",
                        params![new_expiry, holder_json, intent_id, path],
                    )?;
                    return Ok(None);
                } else {
                    return Ok(Some(existing_record));
                }
            }
        }

        let holder_json = serde_json::to_string(holder).unwrap_or_default();
        self.conn.execute(
            "INSERT INTO locks (path, holder, since, expires, intent)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(path) DO UPDATE SET
                holder = excluded.holder,
                since = excluded.since,
                expires = excluded.expires,
                intent = excluded.intent",
            params![path, holder_json, now_str, new_expiry, intent_id],
        )?;

        Ok(None)
    }

    pub fn renew_session_locks(&self, session: &str, ttl_secs: u64) -> Result<()> {
        let now = crate::clock::now();
        let now_str = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let new_expiry = (now + chrono::Duration::seconds(ttl_secs as i64)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let mut stmt = self.conn.prepare("SELECT path, holder FROM locks WHERE expires > ?1")?;
        let rows = stmt.query_map(params![now_str], |row| {
            let holder_str: String = row.get(1)?;
            let holder: Actor =
                serde_json::from_str(&holder_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
            Ok((row.get::<_, String>(0)?, holder))
        })?;

        for item in rows {
            let (path, holder) = item?;
            if holder.session.as_deref() == Some(session) {
                self.conn.execute("UPDATE locks SET expires = ?1 WHERE path = ?2", params![new_expiry, path])?;
            }
        }

        Ok(())
    }

    pub fn release_session_locks(&self, session: &str) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT path, holder FROM locks")?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let holder_str: String = row.get(1)?;
            let holder: Actor =
                serde_json::from_str(&holder_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
            Ok((path, holder))
        })?;

        let mut to_delete = Vec::new();
        for item in rows {
            let (path, holder) = item?;
            if holder.session.as_deref() == Some(session) {
                to_delete.push(path);
            }
        }

        for path in to_delete {
            self.conn.execute("DELETE FROM locks WHERE path = ?1", params![path])?;
        }

        Ok(())
    }

    pub fn release_lock_path(&self, path: &str) -> Result<bool> {
        let count = self.conn.execute("DELETE FROM locks WHERE path = ?1", params![path])?;
        Ok(count > 0)
    }

    pub fn release_lock_path_with_flags(&self, path: &str, work_tree: &Path) -> Result<bool> {
        let _ = work_tree;
        self.with_write_lock(|| self.release_lock_path(path))
    }

    pub fn release_session_locks_with_flags(&self, session: &str, work_tree: &Path) -> Result<usize> {
        self.with_write_lock(|| self.release_session_locks_unlocked(session, work_tree))
    }

    fn release_session_locks_unlocked(&self, session: &str, work_tree: &Path) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT path, holder FROM locks")?;
        let rows = stmt.query_map([], |row| {
            let path: String = row.get(0)?;
            let holder_str: String = row.get(1)?;
            let holder: Actor =
                serde_json::from_str(&holder_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
            Ok((path, holder))
        })?;

        let mut to_delete = Vec::new();
        for item in rows {
            let (path, holder) = item?;
            if holder.session.as_deref() == Some(session) {
                to_delete.push(path);
            }
        }

        let _ = work_tree;
        for path in &to_delete {
            self.conn.execute("DELETE FROM locks WHERE path = ?1", params![path])?;
        }

        Ok(to_delete.len())
    }

    pub fn find_stuck_immutable_files(&self, _work_tree: &Path) -> Vec<String> {
        Vec::new()
    }

    pub fn release_all_locks_with_flags(&self, _work_tree: &Path) -> Result<usize> {
        self.with_write_lock(|| self.conn.execute("DELETE FROM locks", []))
    }

    pub fn clean_expired_locks(&self, work_tree: &Path) -> Result<usize> {
        let now_str = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut stmt = self.conn.prepare("SELECT path FROM locks WHERE expires <= ?1")?;
        let expired_paths: Vec<String> = stmt.query_map(params![now_str], |row| row.get(0))?.filter_map(|r| r.ok()).collect();

        if !expired_paths.is_empty() {
            self.with_write_lock(|| {
                self.conn.execute("DELETE FROM locks WHERE expires <= ?1", params![now_str])?;
                Ok(())
            })?;
        }
        let _ = work_tree;
        Ok(expired_paths.len())
    }

    pub fn get_active_locks(&self) -> Result<Vec<LockRecord>> {
        let now_str = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut stmt = self.conn.prepare("SELECT path, holder, since, expires, intent FROM locks WHERE expires > ?1 ORDER BY path")?;
        let rows = stmt.query_map(params![now_str], |row| {
            let holder_str: String = row.get(1)?;
            let holder: Actor =
                serde_json::from_str(&holder_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
            Ok(LockRecord { path: row.get(0)?, holder, since: row.get(2)?, expires: row.get(3)?, intent: row.get(4)? })
        })?;

        let mut result = Vec::new();
        for item in rows {
            result.push(item?);
        }
        Ok(result)
    }

    pub fn count_active_locks(&self) -> Result<usize> {
        let now_str = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM locks WHERE expires > ?1", params![now_str], |row| row.get(0))?;
        Ok(count as usize)
    }

    pub fn get_lock_for_path(&self, path: &str) -> Result<Option<LockRecord>> {
        let now_str = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut stmt = self.conn.prepare("SELECT path, holder, since, expires, intent FROM locks WHERE path = ?1 AND expires > ?2")?;
        let mut rows = stmt.query(params![path, now_str])?;
        if let Some(row) = rows.next()? {
            let holder_str: String = row.get(1)?;
            let holder: Actor =
                serde_json::from_str(&holder_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
            Ok(Some(LockRecord { path: row.get(0)?, holder, since: row.get(2)?, expires: row.get(3)?, intent: row.get(4)? }))
        } else {
            Ok(None)
        }
    }

    /// When this session last prompted (read only; `get_and_update_last_prompt` is the one that moves it).
    pub fn last_prompt_at(&self, session: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT last_prompt_at FROM notices WHERE session = ?1")?;
        let mut rows = stmt.query(params![session])?;
        rows.next()?.map(|r| r.get(0)).transpose()
    }

    pub fn get_and_update_last_prompt(&self, session: &str, prompt_time: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT last_prompt_at FROM notices WHERE session = ?1")?;
        let mut rows = stmt.query(params![session])?;
        let prev = if let Some(row) = rows.next()? { Some(row.get::<_, String>(0)?) } else { None };

        self.conn.execute(
            "INSERT INTO notices (session, last_prompt_at) VALUES (?1, ?2)
             ON CONFLICT(session) DO UPDATE SET last_prompt_at = excluded.last_prompt_at",
            params![session, prompt_time],
        )?;

        Ok(prev)
    }

    pub fn get_session_declared_or_held_paths(&self, session: &str) -> Result<Vec<String>> {
        let mut paths = std::collections::HashSet::new();

        let intents = self.list_intents(Some(session))?;
        for intent in intents {
            if intent.actor.session.as_deref() == Some(session) {
                for p in intent.paths {
                    paths.insert(p);
                }
            }
        }

        let locks = self.get_active_locks()?;
        for lock in locks {
            if lock.holder.session.as_deref() == Some(session) {
                paths.insert(lock.path);
            }
        }

        let mut stmt = self.conn.prepare("SELECT DISTINCT path FROM changes WHERE actor_session = ?1")?;
        let rows = stmt.query_map(params![session], |row| row.get::<_, String>(0))?;
        for p in rows {
            paths.insert(p?);
        }

        Ok(paths.into_iter().collect())
    }
}
