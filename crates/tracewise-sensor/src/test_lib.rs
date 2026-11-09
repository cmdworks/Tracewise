use super::*;

const RENAME: &str = r#"{"event":{"rename":{"destination_type":1,"destination":{"new_path":{"dir":{"path":"\/r\/proj"},"filename":"a.txt"}},"source":{"path":"\/r\/proj\/a.txt.tmp"}}},"time":"2026-10-24T10:12:11.873Z","process":{"executable":{"path":"\/usr\/bin\/sed"},"audit_token":{"pid":42,"pidversion":7,"euid":501},"ppid":41,"original_ppid":41,"signing_id":"com.apple.sed","team_id":null}}"#;

fn needles() -> Vec<String> {
    prefix_needles("/r/proj")
}

#[test]
fn escaped_slashes_are_found_and_a_sibling_folder_is_not() {
    assert!(mentions_prefix(RENAME, &needles()));
    assert!(!mentions_prefix(RENAME, &prefix_needles("/r/other")));
    assert!(under("/r/proj/a", "/r/proj") && under("/r/proj", "/r/proj/"));
    assert!(!under("/r/projx/a", "/r/proj"));
}

#[test]
fn a_rename_names_its_writer_and_both_paths() {
    let Parsed::Event(ev) = process_line(RENAME, &needles(), &["/r/proj".into()], LIGHT_EVENTS) else {
        panic!("not parsed");
    };
    assert_eq!(ev.kind, "rename");
    assert_eq!(ev.paths, vec!["/r/proj/a.txt", "/r/proj/a.txt.tmp"]);
    assert_eq!(ev.writer.exe, "/usr/bin/sed");
    assert_eq!((ev.writer.pid, ev.writer.pidversion, ev.writer.ppid), (42, 7, Some(41)));
    assert_eq!(ev.writer.signing_id.as_deref(), Some("com.apple.sed"));
    let wire = serde_json::to_string(&ev).unwrap();
    assert!(wire.len() < 400, "compact: {wire}");
    assert_eq!(serde_json::from_str::<SensorEvent>(&wire).unwrap(), ev);
}

#[test]
fn what_is_not_ours_is_dropped_or_skipped_never_fatal() {
    let exec = RENAME.replace("\"rename\"", "\"exec\"");
    assert_eq!(parse_line(&exec, LIGHT_EVENTS), Parsed::Ignored);
    assert_eq!(parse_line("not json", LIGHT_EVENTS), Parsed::Skipped);
    assert_eq!(parse_line("{}", LIGHT_EVENTS), Parsed::Skipped);
    let no_process = r#"{"event":{"rename":{"source":{"path":"\/r\/proj\/a"}}}}"#;
    assert_eq!(parse_line(no_process, LIGHT_EVENTS), Parsed::Skipped);
    // outside the watched folder: dropped before parsing
    assert_eq!(process_line(RENAME, &prefix_needles("/elsewhere"), &["/elsewhere".into()], LIGHT_EVENTS), Parsed::Ignored);
    // `write` only in the full preset
    let write = RENAME.replace("\"rename\"", "\"write\"");
    assert_eq!(parse_line(&write, LIGHT_EVENTS), Parsed::Ignored);
    assert!(matches!(parse_line(&write, FULL_EVENTS), Parsed::Event(_)));
}

#[test]
fn clone_and_link_join_directory_and_name() {
    let clone = r#"{"event":{"clone":{"target_dir":{"path":"\/r\/proj"},"source":{"path":"\/r\/proj\/s.txt"},"target_name":"c.txt"}},"process":{"executable":{"path":"\/bin\/cp"},"audit_token":{"pid":5}}}"#;
    let Parsed::Event(ev) = parse_line(clone, LIGHT_EVENTS) else { panic!() };
    assert_eq!(ev.paths, vec!["/r/proj/c.txt", "/r/proj/s.txt"]);
}

#[test]
fn messages_round_trip() {
    let hello = ToSensor::Hello { v: 1, prefixes: vec!["/r/proj".into()], events: "light".into() };
    let s = serde_json::to_string(&hello).unwrap();
    assert!(s.contains("\"cmd\":\"hello\""));
    assert_eq!(serde_json::from_str::<ToSensor>(&s).unwrap(), hello);
    let st = FromSensor::Status { st: Status { running: true, seen: 9, ..Status::default() } };
    let s = serde_json::to_string(&st).unwrap();
    assert_eq!(serde_json::from_str::<FromSensor>(&s).unwrap(), st);
}
