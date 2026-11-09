//! Compact run-length encoded line attribution map.
//!
//! Replaces the per-line `lines` table with a compact representation:
//! each file has a single row in `line_map(path, runs BLOB)`.
//! The BLOB stores varint-encoded pairs `(change_seq: u32, count: u32)`.
//! Untouched files store 0 rows (missing row = baseline / seq 0).

use similar::{Algorithm, DiffOp, TextDiff};
use std::io;

/// A run of consecutive lines attributed to the same local file change sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineMapRun {
    pub change_seq: u32,
    pub count: u32,
}

impl LineMapRun {
    pub fn new(change_seq: u32, count: u32) -> Self {
        Self { change_seq, count }
    }
}

/// Compact run-length encoded map of line ownership.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineMap {
    pub runs: Vec<LineMapRun>,
}

impl LineMap {
    pub fn new() -> Self {
        Self { runs: Vec::new() }
    }

    /// Total number of lines represented by this line map.
    pub fn len(&self) -> usize {
        self.runs.iter().map(|r| r.count as usize).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.runs.is_empty() || self.len() == 0
    }

    /// Appends lines attributed to `change_seq`, merging with the previous run if possible.
    pub fn push_run(&mut self, change_seq: u32, count: u32) {
        if count == 0 {
            return;
        }
        if let Some(last) = self.runs.last_mut() {
            if last.change_seq == change_seq {
                last.count = last.count.saturating_add(count);
                return;
            }
        }
        self.runs.push(LineMapRun { change_seq, count });
    }

    /// Constructs a `LineMap` from raw runs, merging adjacent matching runs and filtering 0-counts.
    pub fn from_runs(runs: impl IntoIterator<Item = LineMapRun>) -> Self {
        let mut map = Self::new();
        for r in runs {
            map.push_run(r.change_seq, r.count);
        }
        map
    }

    /// Constructs a `LineMap` from a slice of per-line `change_seq`s.
    pub fn from_line_seqs(seqs: &[u32]) -> Self {
        let mut map = Self::new();
        for &s in seqs {
            map.push_run(s, 1);
        }
        map
    }

    /// Expands the runs into a flat vector of `change_seq` for every line.
    pub fn to_line_seqs(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.len());
        for r in &self.runs {
            out.resize(out.len() + r.count as usize, r.change_seq);
        }
        out
    }

    /// Appends the slice `[start, start + len)` of lines from `self` into `target`.
    /// Missing lines (beyond `self.len()`) are treated as baseline (`change_seq: 0`).
    pub fn slice_into(&self, start: usize, len: usize, target: &mut Self) {
        if len == 0 {
            return;
        }
        let mut remaining_skip = start;
        let mut remaining_take = len;

        for r in &self.runs {
            if remaining_take == 0 {
                break;
            }
            let count = r.count as usize;
            if remaining_skip >= count {
                remaining_skip -= count;
                continue;
            }
            let available = count - remaining_skip;
            let take = available.min(remaining_take);
            target.push_run(r.change_seq, take as u32);
            remaining_skip = 0;
            remaining_take -= take;
        }

        // If the requested range extends beyond existing runs, attribute to baseline (0)
        if remaining_take > 0 {
            target.push_run(0, remaining_take as u32);
        }
    }

    /// Encodes the line map into compact varint bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.runs.len() * 4 + 4);
        encode_varint(self.runs.len() as u32, &mut buf);
        for r in &self.runs {
            encode_varint(r.change_seq, &mut buf);
            encode_varint(r.count, &mut buf);
        }
        buf
    }

    /// Decodes compact varint bytes into a `LineMap`.
    pub fn decode(mut bytes: &[u8]) -> io::Result<Self> {
        if bytes.is_empty() {
            return Ok(Self::new());
        }
        let num_runs = decode_varint(&mut bytes)?;
        let mut map = Self { runs: Vec::with_capacity(num_runs as usize) };
        for _ in 0..num_runs {
            let change_seq = decode_varint(&mut bytes)?;
            let count = decode_varint(&mut bytes)?;
            map.push_run(change_seq, count);
        }
        Ok(map)
    }

    /// Applies a text transition (diff) from `old_text` to `new_text`.
    /// New or replaced lines receive `new_change_seq`.
    /// Unmodified lines retain their previous `change_seq` (or 0 for baseline).
    pub fn apply_diff(&self, old_text: Option<&str>, new_text: Option<&str>, new_change_seq: u32) -> Self {
        let new = match new_text {
            Some(n) => n,
            None => return Self::new(),
        };

        let old = old_text.unwrap_or("");
        let old_lines: Vec<&str> = old.lines().collect();
        let new_lines: Vec<&str> = new.lines().collect();

        if new_lines.is_empty() {
            return Self::new();
        }

        if old_lines.is_empty() {
            let mut map = Self::new();
            map.push_run(new_change_seq, new_lines.len() as u32);
            return map;
        }

        let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_slices(&old_lines, &new_lines);

        let mut next = Self::new();

        for op in diff.ops() {
            match *op {
                DiffOp::Equal { old_index, len, .. } => {
                    self.slice_into(old_index, len, &mut next);
                }
                DiffOp::Insert { new_len, .. } => {
                    next.push_run(new_change_seq, new_len as u32);
                }
                DiffOp::Replace { new_len, .. } => {
                    next.push_run(new_change_seq, new_len as u32);
                }
                DiffOp::Delete { .. } => {
                    // Deleted lines are discarded; gaps automatically close.
                }
            }
        }

        next
    }
}

fn encode_varint(mut val: u32, buf: &mut Vec<u8>) {
    loop {
        let byte = (val & 0x7F) as u8;
        val >>= 7;
        if val == 0 {
            buf.push(byte);
            break;
        } else {
            buf.push(byte | 0x80);
        }
    }
}

fn decode_varint(cursor: &mut &[u8]) -> io::Result<u32> {
    let mut result: u32 = 0;
    let mut shift = 0;
    loop {
        if cursor.is_empty() {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "unexpected EOF reading varint"));
        }
        let byte = cursor[0];
        *cursor = &cursor[1..];
        result |= ((byte & 0x7F) as u32) << shift;
        if (byte & 0x80) == 0 {
            return Ok(result);
        }
        shift += 7;
        if shift > 35 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "varint overflow reading LineMap"));
        }
    }
}

#[cfg(test)]
#[path = "test_linemap.rs"]
mod tests;
