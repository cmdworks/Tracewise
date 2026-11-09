use super::Database;
use crate::model::{glob_match, Actor, Intent};
use rusqlite::{params, Result};

impl Database {
    pub fn insert_intent(&self, intent: &Intent) -> Result<()> {
        let actor_json = serde_json::to_string(&intent.actor).unwrap_or_default();
        let paths_json = serde_json::to_string(&intent.paths).unwrap_or_else(|_| "[]".to_string());
        let why = crate::model::mask_secrets(&intent.why);
        self.conn.execute(
            "INSERT INTO intents (id, actor, paths, why, task, created, expires, kind, ended)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![intent.id, actor_json, paths_json, why, intent.task, intent.created, intent.expires, intent.kind, intent.ended,],
        )?;
        Ok(())
    }

    pub fn insert_intent_imported(&self, intent: &Intent) -> Result<bool> {
        let mut check_stmt = self.conn.prepare("SELECT 1 FROM intents WHERE id = ?1")?;
        let mut rows = check_stmt.query(params![intent.id])?;
        if rows.next()?.is_some() {
            return Ok(false);
        }
        self.insert_intent(intent)?;
        Ok(true)
    }

    pub fn get_intent(&self, id: &str) -> Result<Option<Intent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, actor, paths, why, task, created, expires, kind, ended
             FROM intents WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::row_to_intent(row)?))
        } else {
            Ok(None)
        }
    }

    pub fn list_intents(&self, session_filter: Option<&str>) -> Result<Vec<Intent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, actor, paths, why, task, created, expires, kind, ended
             FROM intents WHERE ended = 0 ORDER BY created DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_intent)?;
        let mut list = Vec::new();
        for r in rows {
            let intent = r?;
            if let Some(sess) = session_filter {
                if intent.actor.session.as_deref() != Some(sess) {
                    continue;
                }
            }
            list.push(intent);
        }
        Ok(list)
    }

    pub fn end_session_intents(&self, agent: &str, session: &str) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT id, actor FROM intents WHERE ended = 0")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let actor_str: String = row.get(1)?;
            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            Ok((id, actor))
        })?;

        for r in rows {
            let (id, actor) = r?;
            if actor.name.as_deref() == Some(agent) && actor.session.as_deref() == Some(session) {
                self.conn.execute("UPDATE intents SET ended = 1 WHERE id = ?1", params![id])?;
            }
        }
        Ok(())
    }

    pub fn end_shell_intents(&self, actor: &Actor) -> Result<()> {
        let mut stmt = self.conn.prepare("SELECT id, actor FROM intents WHERE ended = 0 AND kind = 'shell'")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let actor_str: String = row.get(1)?;
            let a: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
            Ok((id, a))
        })?;

        for r in rows {
            let (id, a) = r?;
            if a.name == actor.name {
                self.conn.execute("UPDATE intents SET ended = 1 WHERE id = ?1", params![id])?;
            }
        }
        Ok(())
    }

    pub fn end_intent_by_id(&self, id: &str) -> Result<()> {
        self.conn.execute("UPDATE intents SET ended = 1 WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// True when `agent`'s session has any open, unexpired reason.
    pub fn has_open_intent(&self, agent: &str, session: Option<&str>) -> Result<bool> {
        let now = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        Ok(self.list_intents(None)?.iter().any(|i| {
            i.expires > now
                && i.actor.name.as_deref() == Some(agent)
                && match (&i.actor.session, session) {
                    (None, _) => true,
                    (Some(a), Some(b)) => a == b,
                    (Some(_), None) => false,
                }
        }))
    }

    pub fn find_covering_intent(&self, agent: &str, session: Option<&str>, path: &str) -> Result<Option<Intent>> {
        let now = crate::clock::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let mut stmt = self.conn.prepare(
            "SELECT id, actor, paths, why, task, created, expires, kind, ended
             FROM intents WHERE ended = 0 AND expires > ?1 ORDER BY created DESC",
        )?;
        let rows = stmt.query_map(params![now], Self::row_to_intent)?;
        for r in rows {
            let intent = r?;
            if intent.actor.name.as_deref() == Some(agent) {
                let session_matches = match (&intent.actor.session, session) {
                    (None, _) => true,
                    (Some(s1), Some(s2)) => s1 == s2,
                    (Some(_), None) => false,
                };
                if session_matches {
                    for p in &intent.paths {
                        if glob_match(p, path) {
                            return Ok(Some(intent));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn find_any_covering_intent(&self, agent: &str, session: Option<&str>, path: &str) -> Result<Option<Intent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, actor, paths, why, task, created, expires, kind, ended
             FROM intents ORDER BY created DESC",
        )?;
        let rows = stmt.query_map([], Self::row_to_intent)?;
        for r in rows {
            let intent = r?;
            if intent.actor.name.as_deref() == Some(agent) {
                let session_matches = match (&intent.actor.session, session) {
                    (None, _) => true,
                    (Some(s1), Some(s2)) => s1 == s2,
                    (Some(_), None) => false,
                };
                if session_matches {
                    for p in &intent.paths {
                        if glob_match(p, path) {
                            return Ok(Some(intent));
                        }
                    }
                }
            }
        }
        Ok(None)
    }

    pub fn link_intent_to_change(&self, change_id: &str, intent_id: &str) -> Result<()> {
        self.conn.execute("UPDATE changes SET intent = ?1 WHERE id = ?2", params![intent_id, change_id])?;
        Ok(())
    }

    pub(crate) fn row_to_intent(row: &rusqlite::Row) -> rusqlite::Result<Intent> {
        let id: String = row.get(0)?;
        let actor_str: String = row.get(1)?;
        let paths_str: String = row.get(2)?;
        let why: String = row.get(3)?;
        let task: Option<String> = row.get(4)?;
        let created: String = row.get(5)?;
        let expires: String = row.get(6)?;
        let kind: String = row.get(7)?;
        let ended: i32 = row.get(8)?;

        let actor: Actor = serde_json::from_str(&actor_str).unwrap_or_else(|_| Actor::unknown());
        let paths: Vec<String> = serde_json::from_str(&paths_str).unwrap_or_default();

        Ok(Intent { id, actor, paths, why, task, created, expires, kind, ended })
    }
}

#[cfg(test)]
#[path = "test_intents.rs"]
mod tests;
