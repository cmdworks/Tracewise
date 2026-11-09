use super::Database;
use crate::model::{Actor, CommandRecord};
use rusqlite::{params, Result};
use std::path::Path;

/// Parses a retention string into a duration (e.g. "30d" -> 30 days, "7d" -> 7 days).
/// Supported units: 'd' (days), 'h' (hours), 'w' (weeks), 'm' (30 days), 'y' (365 days).
/// If "all", "none", "off", or "0", returns None (retention disabled).
pub fn parse_retention_duration(v: &str) -> Option<chrono::Duration> {
    let trimmed = v.trim();
    if trimmed.eq_ignore_ascii_case("all") || trimmed.eq_ignore_ascii_case("none") || trimmed.eq_ignore_ascii_case("off") || trimmed == "0"
    {
        return None;
    }
    let (num_part, unit) = if let Some(last) = trimmed.chars().last() {
        if last.is_alphabetic() {
            (&trimmed[..trimmed.len() - 1], last.to_ascii_lowercase())
        } else {
            (trimmed, 'd')
        }
    } else {
        return Some(chrono::Duration::days(30));
    };
    let n: i64 = num_part.trim().parse().ok()?;
    if n <= 0 {
        return None;
    }
    match unit {
        'd' => Some(chrono::Duration::days(n)),
        'h' => Some(chrono::Duration::hours(n)),
        'w' => Some(chrono::Duration::weeks(n)),
        'm' => Some(chrono::Duration::days(n * 30)),
        'y' => Some(chrono::Duration::days(n * 365)),
        _ => Some(chrono::Duration::days(n)),
    }
}

/// Reads the command retention duration from `.trailconfig` in `root`.
/// Defaults to 30 days if not set or invalid.
pub fn read_command_retention(root: &Path) -> Option<chrono::Duration> {
    let val = crate::reasons::config_value(root, "retention", "commands");
    match val.as_deref() {
        None => Some(chrono::Duration::days(30)),
        Some(v) => parse_retention_duration(v),
    }
}

impl Database {
    pub fn insert_command(&self, cmd: &CommandRecord) -> Result<()> {
        let changes_json = serde_json::to_string(&cmd.changes).unwrap_or_else(|_| "[]".to_string());
        let actor_json = serde_json::to_string(&cmd.actor).unwrap_or_else(|_| "{}".to_string());

        // Cap command text at 1024 bytes
        let capped_text = crate::model::cap_command_text(&cmd.text);

        self.conn.execute(
            "INSERT INTO commands (
                id, kind, text, cwd, actor, start, end, exit_code, changes
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                kind = excluded.kind,
                text = excluded.text,
                cwd = excluded.cwd,
                actor = excluded.actor,
                start = excluded.start,
                end = excluded.end,
                exit_code = excluded.exit_code,
                changes = excluded.changes",
            params![cmd.id, cmd.kind, capped_text, cmd.cwd, actor_json, cmd.start, cmd.end, cmd.exit_code, changes_json,],
        )?;
        Ok(())
    }

    /// Names `culprit` as the actor in recent tamper commands that match `path`.
    pub fn name_recent_tamper(&self, path: &str, culprit: &Actor) -> Result<usize> {
        let needle = format!("the lock flag on {} was removed", path);
        let actor_json = serde_json::to_string(culprit).unwrap_or_default();
        let cutoff = (crate::clock::now() - chrono::Duration::seconds(5)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.conn.execute(
            "UPDATE commands SET actor = ?1 WHERE kind = 'tamper' AND start >= ?2 AND instr(text, ?3) > 0",
            params![actor_json, cutoff, needle],
        )
    }

    pub fn insert_command_imported(&self, cmd: &CommandRecord) -> Result<bool> {
        let mut check_stmt = self.conn.prepare("SELECT 1 FROM commands WHERE id = ?1")?;
        let mut rows = check_stmt.query(params![cmd.id])?;
        if rows.next()?.is_some() {
            return Ok(false);
        }
        self.insert_command(cmd)?;
        Ok(true)
    }

    pub fn list_commands(&self) -> Result<Vec<CommandRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, kind, text, cwd, actor, start, end, exit_code, changes
             FROM commands
             ORDER BY start ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let actor_str: String = row.get(4)?;
            let actor: Actor = serde_json::from_str(&actor_str).unwrap_or(Actor { kind: "unknown".to_string(), name: None, session: None });
            let changes_str: String = row.get(8)?;
            let changes: Vec<String> = serde_json::from_str(&changes_str).unwrap_or_default();
            Ok(CommandRecord {
                id: row.get(0)?,
                kind: row.get(1)?,
                text: row.get(2)?,
                cwd: row.get(3)?,
                actor,
                start: row.get(5)?,
                end: row.get(6)?,
                exit_code: row.get(7)?,
                changes,
            })
        })?;

        let mut commands = Vec::new();
        for r in rows {
            commands.push(r?);
        }
        Ok(commands)
    }

    pub fn link_changes_to_command(&self, change_ids: &[String], command_id: &str) -> Result<()> {
        let mut stmt = self.conn.prepare("UPDATE changes SET command = ?1 WHERE id = ?2")?;
        for cid in change_ids {
            stmt.execute(params![command_id, cid])?;
        }
        Ok(())
    }

    /// Prunes command records whose start timestamp is older than `cutoff` (RFC 3339 string).
    /// Returns the number of pruned rows.
    pub fn purge_expired_commands(&self, cutoff: &str) -> Result<usize> {
        let n = self.conn.execute("DELETE FROM commands WHERE start < ?1", params![cutoff])?;
        Ok(n)
    }

    /// Prunes command records older than `retention` duration before `now`.
    /// Returns the number of pruned rows.
    pub fn purge_commands_older_than(&self, retention: chrono::Duration) -> Result<usize> {
        let cutoff = (crate::clock::now() - retention).to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.purge_expired_commands(&cutoff)
    }
}

#[cfg(test)]
#[path = "test_commands.rs"]
mod tests;
