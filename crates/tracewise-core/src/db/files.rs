use super::Database;
use crate::model::FileRecord;
use rusqlite::{params, Result};
use std::collections::HashMap;

impl Database {
    pub fn get_known_files(&self) -> Result<HashMap<String, FileRecord>> {
        let mut stmt = self.conn.prepare("SELECT path, blob, size, mtime_ns FROM files")?;
        let rows =
            stmt.query_map([], |row| Ok(FileRecord { path: row.get(0)?, blob: row.get(1)?, size: row.get(2)?, mtime_ns: row.get(3)? }))?;

        let mut map = HashMap::new();
        for r in rows {
            let record = r?;
            map.insert(record.path.clone(), record);
        }
        Ok(map)
    }

    pub fn get_file(&self, path: &str) -> Result<Option<FileRecord>> {
        let mut stmt = self.conn.prepare("SELECT path, blob, size, mtime_ns FROM files WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(FileRecord { path: row.get(0)?, blob: row.get(1)?, size: row.get(2)?, mtime_ns: row.get(3)? }))
        } else {
            Ok(None)
        }
    }

    pub fn set_file(&self, record: &FileRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO files (path, blob, size, mtime_ns) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(path) DO UPDATE SET blob = ?2, size = ?3, mtime_ns = ?4",
            params![record.path, record.blob, record.size, record.mtime_ns],
        )?;
        Ok(())
    }

    /// Kind and executable bit of every known file (see `migrate_v14`); a file with no row was recorded before schema 14 (its mode is unknown, not changed).
    pub fn get_file_modes(&self) -> Result<HashMap<String, u8>> {
        let mut stmt = self.conn.prepare("SELECT path, mode FROM file_modes")?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u8>(1)?)))?;
        rows.collect()
    }

    pub fn get_file_mode(&self, path: &str) -> Result<Option<u8>> {
        let mut stmt = self.conn.prepare("SELECT mode FROM file_modes WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        rows.next()?.map(|r| r.get::<_, u8>(0)).transpose()
    }

    pub fn set_file_mode(&self, path: &str, mode: u8) -> Result<()> {
        self.conn.execute(
            "INSERT INTO file_modes (path, mode) VALUES (?1, ?2) ON CONFLICT(path) DO UPDATE SET mode = ?2",
            params![path, mode],
        )?;
        Ok(())
    }

    pub fn delete_file(&self, path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM file_modes WHERE path = ?1", params![path])?;
        self.conn.execute("DELETE FROM files WHERE path = ?1", params![path])?;
        Ok(())
    }
}
