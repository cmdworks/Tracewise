use similar::{Algorithm, DiffOp, TextDiff};

/// Updates line change IDs when transitioning from `old_text` to `new_text`.
pub fn update_lines_for_change(
    old_text: Option<&str>,
    new_text: Option<&str>,
    old_line_changes: &[String],
    new_change_id: &str,
) -> Vec<String> {
    let old = old_text.unwrap_or("");
    let new = match new_text {
        Some(n) => n,
        None => return Vec::new(),
    };

    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    if old_lines.is_empty() {
        return vec![new_change_id.to_string(); new_lines.len()];
    }

    let diff = TextDiff::configure().algorithm(Algorithm::Myers).diff_slices(&old_lines, &new_lines);

    let mut result = vec![new_change_id.to_string(); new_lines.len()];

    for op in diff.ops() {
        if let DiffOp::Equal { old_index, new_index, len } = op {
            for i in 0..*len {
                let old_idx = old_index + i;
                let new_idx = new_index + i;
                if old_idx < old_line_changes.len() && new_idx < result.len() {
                    result[new_idx] = old_line_changes[old_idx].clone();
                }
            }
        }
    }

    result
}

#[cfg(test)]
#[path = "test_lines.rs"]
mod tests;
