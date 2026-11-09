use super::*;

#[test]
fn test_push_and_merge() {
    let mut map = LineMap::new();
    map.push_run(1, 5);
    map.push_run(1, 3);
    assert_eq!(map.runs.len(), 1);
    assert_eq!(map.runs[0], LineMapRun::new(1, 8));

    map.push_run(2, 0); // 0 count ignored
    assert_eq!(map.runs.len(), 1);

    map.push_run(2, 4);
    assert_eq!(map.runs.len(), 2);
    assert_eq!(map.runs[1], LineMapRun::new(2, 4));

    map.push_run(1, 2);
    assert_eq!(map.runs.len(), 3);
    assert_eq!(map.len(), 14);
}

#[test]
fn test_encode_decode_roundtrip() {
    let empty = LineMap::new();
    let enc = empty.encode();
    let dec = LineMap::decode(&enc).unwrap();
    assert_eq!(dec, empty);

    let mut map = LineMap::new();
    map.push_run(0, 500);
    map.push_run(1, 10000);
    map.push_run(42, 1);
    map.push_run(200000, 3000);

    let bytes = map.encode();
    let decoded = LineMap::decode(&bytes).unwrap();
    assert_eq!(decoded, map);
    assert_eq!(decoded.to_line_seqs(), map.to_line_seqs());
}

#[test]
fn test_slice_into() {
    let mut map = LineMap::new();
    map.push_run(1, 4); // [0..4)
    map.push_run(2, 6); // [4..10)
    map.push_run(3, 5); // [10..15)

    let mut target = LineMap::new();
    map.slice_into(2, 10, &mut target);
    // Expect: [2..4) from seq 1 (count 2), [4..10) from seq 2 (count 6), [10..12) from seq 3 (count 2)
    assert_eq!(target.runs, vec![LineMapRun::new(1, 2), LineMapRun::new(2, 6), LineMapRun::new(3, 2),]);
    assert_eq!(target.len(), 10);

    // Slice beyond available length fills with 0 (baseline)
    let mut target2 = LineMap::new();
    map.slice_into(12, 10, &mut target2);
    // [12..15) from seq 3 (count 3), [15..22) baseline 0 (count 7)
    assert_eq!(target2.runs, vec![LineMapRun::new(3, 3), LineMapRun::new(0, 7)]);
}

#[test]
fn test_apply_diff_scenarios() {
    // Initial creation
    let v1 = "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\n";
    let map0 = LineMap::new();
    let map1 = map0.apply_diff(None, Some(v1), 1);
    assert_eq!(map1.runs, vec![LineMapRun::new(1, 10)]);

    // Insert at top (L05)
    let v2 = format!("TOP1\nTOP2\n{}", v1);
    let map2 = map1.apply_diff(Some(v1), Some(&v2), 2);
    assert_eq!(map2.runs, vec![LineMapRun::new(2, 2), LineMapRun::new(1, 10)]);

    // Delete lines 4..8 (L06)
    let mut lines: Vec<&str> = v2.lines().collect();
    lines.drain(4..8);
    let v3 = lines.join("\n") + "\n";
    let map3 = map2.apply_diff(Some(&v2), Some(&v3), 3);
    // v2 had: [0..2) seq 2, [2..12) seq 1.
    // lines 0..4: [0..2) seq 2, [2..4) seq 1.
    // lines 4..8 deleted.
    // lines 8..12: [8..12) from v2, which are all seq 1.
    // They merge with seq 1 from [2..4): 2 + 4 = 6 lines!
    assert_eq!(map3.runs, vec![LineMapRun::new(2, 2), LineMapRun::new(1, 6)]);
}

#[test]
fn test_equivalence_with_per_line_model() {
    let mut model: Vec<u32> = vec![0; 20];
    let mut text: Vec<String> = (0..20).map(|i| format!("line {}", i)).collect();
    let mut map = LineMap::from_line_seqs(&model);

    for step in 1..=10 {
        let old_str = text.join("\n") + "\n";
        // Mutate text and model
        if step % 2 == 0 {
            // Delete
            let del_idx = (step * 3) % text.len();
            text.remove(del_idx);
            model.remove(del_idx);
        } else {
            // Insert
            let ins_idx = (step * 2) % (text.len() + 1);
            text.insert(ins_idx, format!("new line at step {}", step));
            model.insert(ins_idx, step as u32);
        }
        let new_str = text.join("\n") + "\n";
        map = map.apply_diff(Some(&old_str), Some(&new_str), step as u32);
        assert_eq!(map.to_line_seqs(), model, "failed equivalence at step {}", step);
    }
}
