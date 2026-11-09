use super::Database;
use crate::linemap::LineMap;
use rusqlite::{params, Result};
use std::collections::HashMap;

impl Database {
    /// Retrieves the raw `LineMap` for `path`, or `None` if the file has never been recorded in `line_map`.
    pub fn get_file_line_map(&self, path: &str) -> Result<Option<LineMap>> {
        let mut stmt = self.conn.prepare("SELECT runs FROM line_map WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let blob: Vec<u8> = row.get(0)?;
            let map = LineMap::decode(&blob).unwrap_or_default();
            Ok(Some(map))
        } else {
            Ok(None)
        }
    }

    /// Retrieves the mapping of `change_seq -> change_id` for a given file path.
    pub fn get_file_changes_map(&self, path: &str) -> Result<HashMap<u32, String>> {
        let mut stmt = self.conn.prepare("SELECT change_seq, change_id FROM file_changes WHERE path = ?1")?;
        let rows = stmt.query_map(params![path], |row| Ok((row.get(0)?, row.get(1)?)))?;
        let mut map = HashMap::new();
        for r in rows {
            let (seq, id) = r?;
            map.insert(seq, id);
        }
        Ok(map)
    }

    /// Stores a file's `LineMap` and its associated `file_changes` entries.
    pub fn set_file_line_map(&mut self, path: &str, line_map: &LineMap, changes: &[(u32, &str)]) -> Result<()> {
        self.conn.execute_batch("SAVEPOINT sp_set_line_map")?;
        let res = (|| -> Result<()> {
            let blob = line_map.encode();
            self.conn.execute("INSERT OR REPLACE INTO line_map (path, runs) VALUES (?1, ?2)", params![path, blob])?;
            let mut stmt =
                self.conn.prepare_cached("INSERT OR REPLACE INTO file_changes (path, change_seq, change_id) VALUES (?1, ?2, ?3)")?;
            for (seq, cid) in changes {
                stmt.execute(params![path, seq, cid])?;
            }
            Ok(())
        })();
        if res.is_ok() {
            self.conn.execute_batch("RELEASE sp_set_line_map")?;
            Ok(())
        } else {
            let _ = self.conn.execute_batch("ROLLBACK TO sp_set_line_map");
            let _ = self.conn.execute_batch("RELEASE sp_set_line_map");
            res
        }
    }

    /// Returns per-line (line_num, change_id) pairs by expanding `line_map` and resolving `file_changes`.
    /// Returns empty if the file has no entry in `line_map`.
    pub fn get_file_lines(&self, path: &str) -> Result<Vec<(usize, String)>> {
        let line_map = match self.get_file_line_map(path)? {
            Some(m) => m,
            None => return Ok(Vec::new()),
        };
        let changes = self.get_file_changes_map(path)?;
        let line_seqs = line_map.to_line_seqs();
        let mut list = Vec::with_capacity(line_seqs.len());
        for (i, seq) in line_seqs.into_iter().enumerate() {
            let change_id =
                if seq == 0 { "baseline".to_string() } else { changes.get(&seq).cloned().unwrap_or_else(|| "baseline".to_string()) };
            list.push((i + 1, change_id));
        }
        Ok(list)
    }

    /// Stores per-line (line_num, change_id) by compressing into `LineMap` and inserting into `file_changes`.
    pub fn set_file_lines(&mut self, path: &str, lines: &[(usize, &str)]) -> Result<()> {
        if lines.is_empty() {
            self.delete_file_lines(path)?;
            return Ok(());
        }

        let mut existing_changes = self.get_file_changes_map(path).unwrap_or_default();
        let mut id_to_seq: HashMap<String, u32> = existing_changes.iter().map(|(&k, v)| (v.clone(), k)).collect();
        let mut max_seq = existing_changes.keys().copied().max().unwrap_or(0);

        let mut line_seqs = Vec::with_capacity(lines.len());
        let mut new_changes = Vec::new();

        for (_, cid) in lines {
            if *cid == "baseline" || cid.is_empty() {
                line_seqs.push(0);
            } else if let Some(&seq) = id_to_seq.get(*cid) {
                line_seqs.push(seq);
            } else {
                max_seq += 1;
                id_to_seq.insert(cid.to_string(), max_seq);
                existing_changes.insert(max_seq, cid.to_string());
                new_changes.push((max_seq, *cid));
                line_seqs.push(max_seq);
            }
        }

        let line_map = LineMap::from_line_seqs(&line_seqs);
        self.set_file_line_map(path, &line_map, &new_changes)
    }

    pub fn delete_file_lines(&self, path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM line_map WHERE path = ?1", params![path])?;
        self.conn.execute("DELETE FROM file_changes WHERE path = ?1", params![path])?;
        Ok(())
    }

    pub fn rename_file_lines(&self, old_path: &str, new_path: &str) -> Result<()> {
        // a rename onto a path that still has rows replaces them; without this the UPDATE hits the primary key
        self.conn.execute("DELETE FROM line_map WHERE path = ?1", params![new_path])?;
        self.conn.execute("DELETE FROM file_changes WHERE path = ?1", params![new_path])?;
        self.conn.execute("UPDATE line_map SET path = ?1 WHERE path = ?2", params![new_path, old_path])?;
        self.conn.execute("UPDATE file_changes SET path = ?1 WHERE path = ?2", params![new_path, old_path])?;
        Ok(())
    }
}
