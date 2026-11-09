use crate::blob::is_binary_or_large;
use crate::model::Hunk;
use similar::TextDiff;

/// Computes hunks between `before` and `after` byte contents.
/// Returns empty hunks if either side is binary/large.
pub fn compute_hunks(before: Option<&[u8]>, after: Option<&[u8]>) -> Vec<Hunk> {
    if before.is_some_and(is_binary_or_large) || after.is_some_and(is_binary_or_large) {
        return Vec::new();
    }

    match (before, after) {
        (None, None) => Vec::new(),
        (None, Some(a)) => {
            let text = String::from_utf8_lossy(a);
            let lines = text.lines().count();
            vec![Hunk { old_start: 0, old_lines: 0, new_start: 1, new_lines: lines }]
        }
        (Some(b), None) => {
            let text = String::from_utf8_lossy(b);
            let lines = text.lines().count();
            vec![Hunk { old_start: 1, old_lines: lines, new_start: 0, new_lines: 0 }]
        }
        (Some(b), Some(a)) => {
            let (before_str, after_str) = (String::from_utf8_lossy(b), String::from_utf8_lossy(a));

            let diff = TextDiff::from_lines(&*before_str, &*after_str);
            let mut hunks = Vec::new();

            for group in diff.grouped_ops(3) {
                if let (Some(first), Some(last)) = (group.first(), group.last()) {
                    let old_start = first.old_range().start + 1;
                    let old_lines = last.old_range().end - first.old_range().start;
                    let new_start = first.new_range().start + 1;
                    let new_lines = last.new_range().end - first.new_range().start;
                    hunks.push(Hunk { old_start, old_lines, new_start, new_lines });
                }
            }

            hunks
        }
    }
}

/// Generates unified diff text between `before` and `after`.
pub fn format_unified_diff(before: Option<&[u8]>, after: Option<&[u8]>, path: &str) -> String {
    if before.is_some_and(is_binary_or_large) || after.is_some_and(is_binary_or_large) {
        return format!("Binary file {path} differs\n");
    }

    let before_str = before.map(String::from_utf8_lossy).unwrap_or_default();
    let after_str = after.map(String::from_utf8_lossy).unwrap_or_default();

    let diff = TextDiff::from_lines(&*before_str, &*after_str);
    let mut out = String::new();
    let header_old = if before.is_some() { format!("a/{path}") } else { "/dev/null".to_string() };
    let header_new = if after.is_some() { format!("b/{path}") } else { "/dev/null".to_string() };

    out.push_str(&diff.unified_diff().header(&header_old, &header_new).to_string());
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub line_in_ours: usize,
    pub base_range: std::ops::Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreeWayMergeResult {
    pub merged_text: String,
    pub conflicts: Vec<MergeConflict>,
}

/// Splits text into lines preserving trailing newlines (e.g. \n or \r\n).
pub fn split_lines_inclusive(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for (i, b) in text.as_bytes().iter().enumerate() {
        if *b == b'\n' {
            lines.push(&text[start..=i]);
            start = i + 1;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

#[derive(Clone, Debug)]
struct Chunk<'a> {
    base_start: usize,
    base_end: usize,
    replacement: Vec<&'a str>,
}

fn collect_diff_chunks<'a>(base_text: &'a str, target_text: &'a str, target_lines: &[&'a str]) -> Vec<Chunk<'a>> {
    let diff = TextDiff::from_lines(base_text, target_text);
    let mut chunks = Vec::new();
    for op in diff.ops() {
        match *op {
            similar::DiffOp::Equal { .. } => {}
            similar::DiffOp::Delete { old_index, old_len, .. } => {
                chunks.push(Chunk { base_start: old_index, base_end: old_index + old_len, replacement: Vec::new() });
            }
            similar::DiffOp::Insert { old_index, new_index, new_len } => {
                chunks.push(Chunk {
                    base_start: old_index,
                    base_end: old_index,
                    replacement: target_lines[new_index..new_index + new_len].to_vec(),
                });
            }
            similar::DiffOp::Replace { old_index, old_len, new_index, new_len } => {
                chunks.push(Chunk {
                    base_start: old_index,
                    base_end: old_index + old_len,
                    replacement: target_lines[new_index..new_index + new_len].to_vec(),
                });
            }
        }
    }
    chunks
}

/// One side's text for base lines `start..end`: its chunks applied, with the untouched base lines between and around them kept.
/// Using only the chunks' replacements would drop those base lines from the merged file.
fn apply_chunks<'a>(base_lines: &[&'a str], chunks: &[Chunk<'a>], start: usize, end: usize) -> Vec<&'a str> {
    let end = end.min(base_lines.len());
    let mut out = Vec::new();
    let mut at = start;
    for c in chunks {
        if at < c.base_start {
            out.extend_from_slice(&base_lines[at..c.base_start.min(end)]);
        }
        out.extend_from_slice(&c.replacement);
        at = c.base_end;
    }
    if at < end {
        out.extend_from_slice(&base_lines[at..end]);
    }
    out
}

/// The new text of each base line a side rewrote in place, or `None` when the side added or removed lines.
fn edited_lines<'a>(base_lines: &[&'a str], chunks: &[Chunk<'a>]) -> Option<std::collections::HashMap<usize, &'a str>> {
    let mut map = std::collections::HashMap::new();
    for c in chunks {
        if c.base_end <= c.base_start || c.base_end - c.base_start != c.replacement.len() || c.base_end > base_lines.len() {
            return None;
        }
        for (k, line) in c.replacement.iter().enumerate() {
            map.insert(c.base_start + k, *line);
        }
    }
    Some(map)
}

type EditedLines<'a> = std::collections::HashMap<usize, &'a str>;

/// Both sides only rewrote lines in place: settle each line, so only the lines both rewrote are a conflict.
fn settle_rewritten_lines<'a>(
    base_lines: &[&'a str],
    region: std::ops::Range<usize>,
    (theirs, ours): (&EditedLines<'a>, &EditedLines<'a>),
    result_lines: &mut Vec<&'a str>,
    conflicts: &mut Vec<MergeConflict>,
) {
    let mut run: Option<(usize, usize)> = None;
    for (i, &b) in base_lines.iter().enumerate().take(region.end).skip(region.start) {
        let (t, o) = (theirs.get(&i).copied().unwrap_or(b), ours.get(&i).copied().unwrap_or(b));
        let both = t != b && o != b && t != o;
        if both && run.is_none() {
            run = Some((result_lines.len() + 1, i));
        } else if !both {
            if let Some((line_in_ours, from)) = run.take() {
                conflicts.push(MergeConflict { line_in_ours, base_range: from..i });
            }
        }
        result_lines.push(if t == b || t == o || both { o } else { t });
    }
    if let Some((line_in_ours, from)) = run {
        conflicts.push(MergeConflict { line_in_ours, base_range: from..region.end });
    }
}

/// Performs a line-level three-way merge applying changes from base -> theirs onto ours.
/// When conflicts occur between theirs and ours, ours is kept and a MergeConflict is recorded.
pub fn three_way_merge(base_text: &str, ours_text: &str, theirs_text: &str) -> ThreeWayMergeResult {
    if ours_text == base_text {
        return ThreeWayMergeResult { merged_text: theirs_text.to_string(), conflicts: Vec::new() };
    }
    if theirs_text == base_text || ours_text == theirs_text {
        return ThreeWayMergeResult { merged_text: ours_text.to_string(), conflicts: Vec::new() };
    }

    let base_lines = split_lines_inclusive(base_text);
    let ours_lines = split_lines_inclusive(ours_text);
    let theirs_lines = split_lines_inclusive(theirs_text);
    let theirs_chunks = collect_diff_chunks(base_text, theirs_text, &theirs_lines);
    let ours_chunks = collect_diff_chunks(base_text, ours_text, &ours_lines);

    let mut result_lines = Vec::new();
    let mut conflicts = Vec::new();

    let mut t_idx = 0;
    let mut o_idx = 0;
    let mut curr_base = 0;

    while t_idx < theirs_chunks.len() || o_idx < ours_chunks.len() {
        let t_chunk = theirs_chunks.get(t_idx);
        let o_chunk = ours_chunks.get(o_idx);

        let (t_start, o_start) = match (t_chunk, o_chunk) {
            (Some(t), Some(o)) => (t.base_start, o.base_start),
            (Some(t), None) => (t.base_start, usize::MAX),
            (None, Some(o)) => (usize::MAX, o.base_start),
            (None, None) => break,
        };

        let overlaps = match (t_chunk, o_chunk) {
            (Some(t), Some(o)) => {
                if t.base_start == o.base_start && t.base_end == t.base_start && o.base_end == o.base_start {
                    true
                } else {
                    t.base_start < o.base_end && o.base_start < t.base_end
                }
            }
            _ => false,
        };

        if overlaps {
            let reg_base_start = t_start.min(o_start);
            let mut reg_base_end = t_chunk.unwrap().base_end.max(o_chunk.unwrap().base_end);

            let t_sub_start = t_idx;
            let o_sub_start = o_idx;
            t_idx += 1;
            o_idx += 1;

            loop {
                let mut expanded = false;
                if t_idx < theirs_chunks.len() && theirs_chunks[t_idx].base_start <= reg_base_end {
                    reg_base_end = reg_base_end.max(theirs_chunks[t_idx].base_end);
                    t_idx += 1;
                    expanded = true;
                }
                if o_idx < ours_chunks.len() && ours_chunks[o_idx].base_start <= reg_base_end {
                    reg_base_end = reg_base_end.max(ours_chunks[o_idx].base_end);
                    o_idx += 1;
                    expanded = true;
                }
                if !expanded {
                    break;
                }
            }

            if curr_base < reg_base_start && reg_base_start <= base_lines.len() {
                result_lines.extend_from_slice(&base_lines[curr_base..reg_base_start]);
            }

            let theirs_repl = apply_chunks(&base_lines, &theirs_chunks[t_sub_start..t_idx], reg_base_start, reg_base_end);
            let ours_repl = apply_chunks(&base_lines, &ours_chunks[o_sub_start..o_idx], reg_base_start, reg_base_end);

            if theirs_repl == ours_repl {
                result_lines.extend(theirs_repl);
            } else if let (Some(tm), Some(om)) =
                (edited_lines(&base_lines, &theirs_chunks[t_sub_start..t_idx]), edited_lines(&base_lines, &ours_chunks[o_sub_start..o_idx]))
            {
                settle_rewritten_lines(&base_lines, reg_base_start..reg_base_end, (&tm, &om), &mut result_lines, &mut conflicts);
            } else {
                let line_in_ours = result_lines.len() + 1;
                conflicts.push(MergeConflict { line_in_ours, base_range: reg_base_start..reg_base_end });
                result_lines.extend(ours_repl);
            }
            curr_base = reg_base_end;
        } else if t_start < o_start {
            let t = t_chunk.unwrap();
            if curr_base < t.base_start && t.base_start <= base_lines.len() {
                result_lines.extend_from_slice(&base_lines[curr_base..t.base_start]);
            }
            result_lines.extend_from_slice(&t.replacement);
            curr_base = t.base_end;
            t_idx += 1;
        } else {
            let o = o_chunk.unwrap();
            if curr_base < o.base_start && o.base_start <= base_lines.len() {
                result_lines.extend_from_slice(&base_lines[curr_base..o.base_start]);
            }
            result_lines.extend_from_slice(&o.replacement);
            curr_base = o.base_end;
            o_idx += 1;
        }
    }

    if curr_base < base_lines.len() {
        result_lines.extend_from_slice(&base_lines[curr_base..]);
    }

    ThreeWayMergeResult { merged_text: result_lines.concat(), conflicts }
}

#[cfg(test)]
#[path = "test_diff.rs"]
mod tests;
