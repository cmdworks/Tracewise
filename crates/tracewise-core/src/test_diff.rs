use super::*;

#[test]
fn test_compute_hunks_create() {
    let hunks = compute_hunks(None, Some(b"one\ntwo\n"));
    assert_eq!(hunks.len(), 1);
    assert_eq!(hunks[0].new_lines, 2);
}

#[test]
fn test_compute_hunks_modify() {
    let hunks = compute_hunks(Some(b"one\ntwo\n"), Some(b"one\nTWO\nthree\n"));
    assert!(!hunks.is_empty());
    assert!(hunks[0].old_lines >= 1);
    assert!(hunks[0].new_lines >= 1);
}

#[test]
fn test_format_diff() {
    let diff = format_unified_diff(None, Some(b"hello\n"), "d.txt");
    assert!(diff.contains("+hello"));
}

#[test]
fn test_three_way_merge_reverse_insertion_with_interleaved_edits() {
    let base = "top\na\nb\nCODEX\nc\n";
    let theirs = "top\na\nb\nc\n";
    let ours = "top\na\nb\nCODEX\nc\nbottom\n";

    let res = three_way_merge(base, ours, theirs);
    assert_eq!(res.merged_text, "top\na\nb\nc\nbottom\n");
    assert!(res.conflicts.is_empty());
}

#[test]
fn test_three_way_merge_conflict_on_same_line() {
    let base = "one\nTWO by codex\nthree\n";
    let theirs = "one\ntwo\nthree\n";
    let ours = "one\nTWO by claude\nthree\nextra\n";

    let res = three_way_merge(base, ours, theirs);
    assert_eq!(res.merged_text, "one\nTWO by claude\nthree\nextra\n");
    assert_eq!(res.conflicts.len(), 1);
    assert_eq!(res.conflicts[0].line_in_ours, 2);
}

#[test]
fn test_three_way_merge_line_endings_preserved() {
    let base = "a\r\nb\r\nCODEX\r\n";
    let theirs = "a\r\nb\r\n";
    let ours = "a\r\nb\r\nCODEX\r\n";
    let res = three_way_merge(base, ours, theirs);
    assert_eq!(res.merged_text, "a\r\nb\r\n");

    let base_nonl = "a\nb\nCODEX";
    let theirs_nonl = "a\nb";
    let ours_nonl = "a\nb\nCODEX";
    let res_nonl = three_way_merge(base_nonl, ours_nonl, theirs_nonl);
    assert_eq!(res_nonl.merged_text, "a\nb");
}

fn numbered(from: usize, to: usize, tag: &str) -> String {
    (1..=30).map(|n| if (from..=to).contains(&n) { format!("line {n} {tag}\n") } else { format!("line {n}\n") }).collect()
}

#[test]
fn test_three_way_merge_partial_overlap_keeps_ours_untouched_lines() {
    // step 1 edited 10-15, step 2 edited 12-19; undoing step 1 restores 10-11 and keeps step 2's lines 12-19
    let base = numbered(10, 15, "S1");
    let theirs = numbered(0, 0, "");
    let ours: String = (1..=30)
        .map(|n| match n {
            10..=11 => format!("line {n} S1\n"),
            12..=15 => format!("line {n} S1 S2\n"),
            16..=19 => format!("line {n} S2\n"),
            _ => format!("line {n}\n"),
        })
        .collect();

    let res = three_way_merge(&base, &ours, &theirs);
    let expected: String = (1..=30)
        .map(|n| match n {
            12..=15 => format!("line {n} S1 S2\n"),
            16..=19 => format!("line {n} S2\n"),
            _ => format!("line {n}\n"),
        })
        .collect();
    assert_eq!(res.merged_text, expected);
    assert_eq!(res.conflicts.len(), 1);
}

#[test]
fn test_three_way_merge_second_edit_inside_first_keeps_all_lines() {
    let base = numbered(10, 15, "S1");
    let theirs = numbered(0, 0, "");
    let ours: String = (1..=30)
        .map(|n| match n {
            10..=11 | 14..=15 => format!("line {n} S1\n"),
            12..=13 => format!("line {n} S1 S2\n"),
            _ => format!("line {n}\n"),
        })
        .collect();

    let res = three_way_merge(&base, &ours, &theirs);
    let expected: String =
        (1..=30).map(|n| if (12..=13).contains(&n) { format!("line {n} S1 S2\n") } else { format!("line {n}\n") }).collect();
    assert_eq!(res.merged_text, expected);
    assert_eq!(res.conflicts.len(), 1);
}

#[test]
fn test_three_way_merge_adjacent_edits_both_apply() {
    let base = numbered(10, 12, "S1");
    let theirs = numbered(0, 0, "");
    let ours: String = (1..=30)
        .map(|n| match n {
            10..=12 => format!("line {n} S1\n"),
            13..=15 => format!("line {n} S2\n"),
            _ => format!("line {n}\n"),
        })
        .collect();

    let res = three_way_merge(&base, &ours, &theirs);
    assert!(res.conflicts.is_empty());
    assert_eq!(res.merged_text, numbered(13, 15, "S2"));
}

#[test]
fn undoing_an_overlapped_edit_restores_the_lines_nobody_touched_since() {
    let lines = |f: &dyn Fn(usize) -> String| (1..=30).map(|i| format!("{}\n", f(i))).collect::<String>();
    let before_first = lines(&|i| format!("line {i}"));
    let after_first = lines(&|i| if (10..=15).contains(&i) { format!("A {i}") } else { format!("line {i}") });
    let now = lines(&|i| match i {
        10..=11 => format!("A {i}"),
        12..=19 => format!("B {i}"),
        _ => format!("line {i}"),
    });
    let r = three_way_merge(&after_first, &now, &before_first);
    assert!(r.merged_text.contains("line 10\nline 11\n"), "lines 10-11 come back");
    assert!(r.merged_text.contains("B 12\n") && r.merged_text.contains("B 15\n") && r.merged_text.contains("B 19\n"));
    assert_eq!(r.merged_text.lines().count(), 30);
    assert_eq!(r.conflicts.len(), 1);
    assert_eq!((r.conflicts[0].line_in_ours, r.conflicts[0].base_range.clone()), (12, 11..15));
}
