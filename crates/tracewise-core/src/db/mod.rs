use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};

mod chain;
mod changes;
mod commands;
mod files;
mod intents;
mod lines;
mod locks;
mod prune;
mod windows;

pub use commands::{parse_retention_duration, read_command_retention};
pub use prune::PruneReport;

pub const LATEST_SCHEMA_VERSION: i64 = 16;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct VerifyReport {
    pub ok: bool,
    pub changes: usize,
    pub first_bad: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DatabaseGcReport {
    pub commands_pruned: usize,
}

pub struct Database {
    pub(crate) conn: Connection,
    /// `chain-head` next to the database: the newest record's id and hash, so deleting the newest records
    /// (or the whole database) shows up in `verify`.
    pub(crate) head_path: Option<PathBuf>,
    /// The head of a batch that is not committed yet: written only after `COMMIT`, so a killed scan leaves a head that is still in the chain (Q02).
    pub(crate) pending_head: std::cell::RefCell<Option<String>>,
}

/// SQLite's own busy handler sleeps up to 100 ms between tries, so with ten hooks at once one unlucky process waits a
/// long time for a lock that was free after 2 ms (N11). These wait about 1 ms between tries, with a small spread.
fn wait_for_lock(attempt: i32, max_tries: i32) -> bool {
    if attempt >= max_tries {
        return false;
    }
    std::thread::sleep(std::time::Duration::from_micros(300 + (attempt as u64 * 7919 % 9) * 100));
    true
}

/// Hooks and shell calls: give up after about 250 ms (they fail open).
fn wait_briefly(attempt: i32) -> bool {
    wait_for_lock(attempt, 400)
}

/// Commands that must not lose their work: wait up to about 10 s.
fn wait_patient(attempt: i32) -> bool {
    wait_for_lock(attempt, 15000)
}

impl Database {
    /// Opens the file, makes it owner-only (V02) and refuses a database written by a newer Tracewise without touching it (V16).
    fn open_checked(db_path: &Path) -> Result<Connection> {
        let conn = Connection::open(db_path)?;
        let _ = tracewise_platform::private::strip(db_path);
        if let Some(dir) = db_path.parent() {
            let _ = tracewise_platform::private::strip(dir);
        }
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap_or(0);
        if version > LATEST_SCHEMA_VERSION {
            let msg = format!("this history was written by a newer Tracewise (schema {version}, this Tracewise knows {LATEST_SCHEMA_VERSION}); upgrade Tracewise to use it");
            return Err(rusqlite::Error::ToSqlConversionFailure(msg.into()));
        }
        Ok(conn)
    }

    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Self::open_checked(db_path)?;
        conn.busy_handler(Some(wait_patient))?;
        if conn.pragma_update(None, "journal_mode", "WAL").is_err() {
            // the data folder cannot be written (read-only, or a full disk): reading still works
            return Self::open_read_only(db_path);
        }
        let full = db_path.parent().is_some_and(crate::blob::load_durability);
        conn.pragma_update(None, "synchronous", if full { "FULL" } else { "NORMAL" })?;

        let db = Self { conn, head_path: Some(db_path.with_file_name("chain-head")), pending_head: Default::default() };
        db.migrate()?;
        Ok(db)
    }

    /// A snapshot for reading only, for a history that cannot be written (Q05). Writes then fail with SQLite's own message.
    fn open_read_only(db_path: &Path) -> Result<Self> {
        let encoded: String = db_path
            .to_string_lossy()
            .chars()
            .map(|c| if matches!(c, '%' | '?' | '#') { format!("%{:02X}", c as u32) } else { c.to_string() })
            .collect();
        let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI;
        let conn = Connection::open_with_flags(format!("file:{encoded}?immutable=1"), flags)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap_or(0);
        if version != LATEST_SCHEMA_VERSION {
            let msg = format!("this history is read-only and is at schema {version}, this Tracewise needs {LATEST_SCHEMA_VERSION}; Tracewise does not migrate old histories");
            return Err(rusqlite::Error::ToSqlConversionFailure(msg.into()));
        }
        Ok(Self { conn, head_path: Some(db_path.with_file_name("chain-head")), pending_head: Default::default() })
    }

    /// Fast open for hot path (hooks, shell begin/end): sets timeout without redundant PRAGMAs.
    pub fn open_hot(db_path: &Path) -> Result<Self> {
        let conn = Self::open_checked(db_path)?;
        conn.busy_handler(Some(wait_briefly))?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap_or(0);
        let db = Self { conn, head_path: Some(db_path.with_file_name("chain-head")), pending_head: Default::default() };
        if version < LATEST_SCHEMA_VERSION {
            db.migrate()?;
        }
        Ok(db)
    }

    /// Creates the schema in a new database. Tracewise has no migrations: a history at another schema is refused and must be
    /// recreated (`rm -rf .git/tracewise && tracewise init`).
    fn migrate(&self) -> Result<()> {
        let version: i64 = self.conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version == LATEST_SCHEMA_VERSION {
            return Ok(());
        }
        if version != 0 {
            let msg = format!("this history is at schema {version}, this Tracewise needs {LATEST_SCHEMA_VERSION}; Tracewise does not migrate old histories: remove .git/tracewise and run `tracewise init`");
            return Err(rusqlite::Error::ToSqlConversionFailure(msg.into()));
        }
        self.conn.execute_batch(include_str!("schema.sql"))?;
        self.conn.pragma_update(None, "user_version", LATEST_SCHEMA_VERSION)
    }

    /// Starts a batch holding the write lock from the first statement (a deferred `BEGIN` would let two scans read the
    /// same old state and both record the same change, K05).
    pub fn begin_batch(&self) -> Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")
    }

    /// Before a change record commits, flushes the blobs written for it (`[durability] level = "full"` only).
    fn flush_blobs(&self) -> Result<()> {
        if let Some(dir) = self.head_path.as_deref().and_then(Path::parent) {
            crate::blob::flush_if_pending(dir).map_err(|e| rusqlite::Error::ToSqlConversionFailure(e.into()))?;
        }
        Ok(())
    }

    pub fn commit_batch(&self) -> Result<()> {
        self.flush_blobs()?;
        self.conn.execute_batch("COMMIT")?;
        if let Some(body) = self.pending_head.take() {
            self.write_head_file(&body);
        }
        Ok(())
    }

    pub fn rollback_batch(&self) -> Result<()> {
        self.pending_head.take();
        self.conn.execute_batch("ROLLBACK")
    }

    /// Runs `f` under SQLite's write lock (unless a transaction is already open), so that reading the
    /// latest hash and inserting the next change cannot interleave with another process.
    pub(crate) fn with_write_lock<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        if !self.conn.is_autocommit() {
            return f();
        }
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        let result = f();
        if result.is_ok() {
            self.flush_blobs()?;
        }
        self.conn.execute_batch(if result.is_ok() { "COMMIT" } else { "ROLLBACK" })?;
        result
    }

    /// Database garbage collection and compaction:
    /// 1. Prunes expired command rows according to repo retention (or 30 days default).
    /// 2. Checkpoints the write-ahead log (WAL) with TRUNCATE mode.
    /// 3. Executes VACUUM to reclaim free pages and shrink the database file.
    pub fn gc(&self, work_tree: Option<&Path>) -> Result<DatabaseGcReport> {
        let mut commands_pruned = 0;

        if let Some(wt) = work_tree {
            if let Some(retention) = read_command_retention(wt) {
                commands_pruned += self.purge_commands_older_than(retention)?;
            }
        } else if let Some(retention) = parse_retention_duration("30d") {
            commands_pruned += self.purge_commands_older_than(retention)?;
        }

        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        self.conn.execute_batch("VACUUM;")?;
        self.conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        if let Some(dir) = self.head_path.as_deref().and_then(Path::parent) {
            tracewise_platform::private::strip_tree(dir); // git's own pack files, history written before V02
        }

        Ok(DatabaseGcReport { commands_pruned })
    }
}

#[cfg(test)]
#[path = "test_mod.rs"]
mod tests;
