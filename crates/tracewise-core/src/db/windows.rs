use super::Database;
use crate::model::{Actor, Window};
use rusqlite::{params, Result};

impl Database {
    pub fn open_window(&self, window: &Window) -> Result<()> {
        let actor_json = serde_json::to_string(&window.actor).unwrap_or_default();
        let paths_json = serde_json::to_string(&window.paths).unwrap_or_default();
        let command = window.command.as_deref().map(crate::model::mask_secrets);
        self.conn.execute(
            "INSERT INTO windows (id, kind, actor, paths, cwd, start, end, command) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET kind = ?2, actor = ?3, paths = ?4, cwd = ?5, start = ?6, end = ?7, command = ?8",
            params![window.id, window.kind, actor_json, paths_json, window.cwd, window.start, window.end, command],
        )?;
        Ok(())
    }

    pub fn close_window_by_id(&self, id: &str, end_time: &str) -> Result<Option<Window>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, actor, paths, cwd, start, end, command
             FROM windows WHERE id = ?1 AND end IS NULL",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let actor_str: String = row.get(2)?;
            let paths_str: String = row.get(3)?;
            let cwd: Option<String> = row.get(4)?;
            let start: String = row.get(5)?;
            let _end: Option<String> = row.get(6)?;
            let command: Option<String> = row.get(7)?;

            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            let paths: Vec<String> = serde_json::from_str(&paths_str).unwrap_or_default();

            self.conn.execute("UPDATE windows SET end = ?1 WHERE id = ?2", params![end_time, id])?;

            Ok(Some(Window { id, kind, actor, paths, cwd, start, end: Some(end_time.to_string()), command }))
        } else {
            Ok(None)
        }
    }

    pub fn get_open_windows(&self) -> Result<Vec<Window>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, actor, paths, cwd, start, end, command
             FROM windows WHERE end IS NULL ORDER BY start DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let actor_str: String = row.get(2)?;
            let paths_str: String = row.get(3)?;
            let cwd: Option<String> = row.get(4)?;
            let start: String = row.get(5)?;
            let end: Option<String> = row.get(6)?;
            let command: Option<String> = row.get(7)?;

            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            let paths: Vec<String> = serde_json::from_str(&paths_str).unwrap_or_default();

            Ok(Window { id, kind, actor, paths, cwd, start, end, command })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Windows of work an agent started in the background (written with their expiry as `end`).
    pub fn list_background_windows(&self) -> Result<Vec<Window>> {
        let now = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, actor, paths, cwd, start, end, command
             FROM windows WHERE kind = 'agent_background' AND end > ?1 ORDER BY start ASC",
        )?;
        let rows = stmt.query_map(params![now], |row| {
            let actor_str: String = row.get(2)?;
            let paths_str: String = row.get(3)?;
            Ok(Window {
                id: row.get(0)?,
                kind: row.get(1)?,
                actor: serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown()),
                paths: serde_json::from_str(&paths_str).unwrap_or_default(),
                cwd: row.get(4)?,
                start: row.get(5)?,
                end: row.get(6)?,
                command: row.get(7)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Ends a background window now. False when there is no such live window.
    pub fn stop_background_window(&self, id: &str) -> Result<bool> {
        let now = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let n =
            self.conn.execute("UPDATE windows SET end = ?1 WHERE id = ?2 AND kind = 'agent_background' AND end > ?1", params![now, id])?;
        Ok(n > 0)
    }

    pub fn get_overlapping_windows(&self, start: &str, end: &str) -> Result<Vec<Window>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, actor, paths, cwd, start, end, command
             FROM windows
             WHERE (end IS NULL OR end >= ?1) AND start <= ?2
             ORDER BY start ASC",
        )?;
        let rows = stmt.query_map(params![start, end], |row| {
            let id: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let actor_str: String = row.get(2)?;
            let paths_str: String = row.get(3)?;
            let cwd: Option<String> = row.get(4)?;
            let start: String = row.get(5)?;
            let end: Option<String> = row.get(6)?;
            let command: Option<String> = row.get(7)?;

            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            let paths: Vec<String> = serde_json::from_str(&paths_str).unwrap_or_default();

            Ok(Window { id, kind, actor, paths, cwd, start, end, command })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn close_latest_window(&self, agent_name: &str, session: &str, kind: &str, end_time: &str) -> Result<Option<Window>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, actor, paths, cwd, start, end, command
             FROM windows
             WHERE end IS NULL AND kind = ?1
             ORDER BY start DESC",
        )?;
        let rows = stmt.query_map(params![kind], |row| {
            let id: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let actor_str: String = row.get(2)?;
            let paths_str: String = row.get(3)?;
            let cwd: Option<String> = row.get(4)?;
            let start: String = row.get(5)?;
            let end: Option<String> = row.get(6)?;
            let command: Option<String> = row.get(7)?;

            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            let paths: Vec<String> = serde_json::from_str(&paths_str).unwrap_or_default();

            Ok(Window { id, kind, actor, paths, cwd, start, end, command })
        })?;

        for r in rows {
            let mut w = r?;
            if w.actor.name.as_deref() == Some(agent_name) && w.actor.session.as_deref() == Some(session) {
                self.conn.execute("UPDATE windows SET end = ?1 WHERE id = ?2", params![end_time, w.id])?;
                w.end = Some(end_time.to_string());
                return Ok(Some(w));
            }
        }
        Ok(None)
    }

    pub fn close_session_windows(&self, agent_name: &str, session: &str, end_time: &str) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT id, actor FROM windows WHERE end IS NULL")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let actor_str: String = row.get(1)?;
            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            Ok((id, actor))
        })?;

        let mut closed = 0;
        for r in rows {
            let (id, actor) = r?;
            if actor.name.as_deref() == Some(agent_name) && actor.session.as_deref() == Some(session) {
                self.conn.execute("UPDATE windows SET end = ?1 WHERE id = ?2", params![end_time, id])?;
                closed += 1;
            }
        }
        Ok(closed)
    }

    pub fn has_closed_windows_for_agent(&self, agent_name: &str) -> Result<bool> {
        let mut stmt = self.conn.prepare("SELECT actor FROM windows WHERE end IS NOT NULL")?;
        let rows = stmt.query_map([], |row| {
            let actor_str: String = row.get(0)?;
            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            Ok(actor)
        })?;
        for actor in rows.flatten() {
            if actor.name.as_deref() == Some(agent_name) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[cfg(test)]
#[path = "test_windows.rs"]
mod tests;
