//! Test data generator: writes companion-repository batches straight into a data folder (`<data>/batches/<id>/`), valid,
//! hostile, duplicate or fuzzed, so a test can run `trail rebuild` and check what the program accepts.
//!   cargo run --release -p trail-core --example forge_batch -- <repo> valid|hostile|duplicate|fuzz [seed]
use std::path::{Path, PathBuf};
use tracewise_core::model::{Actor, Change, GitInfo, TimeRange};
use tracewise_core::share::BatchMeta;

fn change(id: &str, path: &str) -> Change {
    let actor = Actor { kind: "human".into(), name: Some("forged".into()), session: None };
    Change {
        id: id.into(),
        time: TimeRange { start: "2026-01-01T00:00:00.000Z".into(), end: "2026-01-01T00:00:00.000Z".into() },
        path: path.into(),
        op: "modify".into(),
        old_path: None,
        hunks: Vec::new(),
        before: None,
        after: None,
        candidates: vec![actor.clone()],
        actor,
        confidence: "exact".into(),
        source: "forge".into(),
        command: None,
        intent: None,
        lock_conflict: None,
        device: "forge".into(),
        git: GitInfo { branch: "main".into(), head: "0".repeat(40) },
        prev_hash: None,
        undoes: None,
    }
}

fn write(data: &Path, id: &str, lines: &[String]) {
    let dir = data.join("batches").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let meta = BatchMeta { id: id.into(), device: "forge".into(), from: "0".into(), to: "1".into(), format: 1, level: "full".into() };
    std::fs::write(dir.join("batch.json"), serde_json::to_string(&meta).unwrap()).unwrap();
    let body = lines.join("\n") + "\n";
    std::fs::write(dir.join("changes.jsonl.zst"), zstd::encode_all(body.as_bytes(), 3).unwrap()).unwrap();
}

fn lines(changes: &[Change]) -> Vec<String> {
    changes.iter().map(|c| serde_json::to_string(c).unwrap()).collect()
}

/// A small deterministic generator (no extra crate): xorshift.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (repo, kind) = (PathBuf::from(&args[0]), args[1].as_str());
    let data = repo.join(".git/trail");
    match kind {
        "valid" => write(
            &data,
            "forge-valid",
            &lines(&(0..5).map(|i| change(&format!("01FORGEVALID{i:014}"), &format!("ok{i}.txt"))).collect::<Vec<_>>()),
        ),
        "hostile" => {
            let evil = ["../x.txt", "/etc/passwd", "a/../../b", "..", ".git/config", ".GIT/trail/trail.db", "a\0b", &"x/".repeat(3000)];
            let changes: Vec<Change> = evil.iter().enumerate().map(|(i, p)| change(&format!("01FORGEEVIL{i:015}"), p)).collect();
            write(&data, "forge-hostile-paths", &lines(&changes));
            write(
                &data,
                "forge-hostile-json",
                &["{\"id\": \"x\", \"path\": ".into(), "{{{ not json".into(), "\u{fffd}\u{fffd}".repeat(50)],
            );
        }
        "duplicate" => {
            write(&data, "forge-dup-a", &lines(&[change("01FORGESAMEID00000000000A", "same.txt")]));
            write(&data, "forge-dup-b", &lines(&[change("01FORGESAMEID00000000000A", "other.txt")]));
        }
        "fuzz" => {
            let mut rng = Rng(args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1) | 1);
            let paths = ["f.txt", "../f", "d/e.txt", "", "a b", "é", ".git/x", "/abs"];
            for n in 0..200 {
                let count = rng.next() % 6;
                let mut ls: Vec<String> = (0..count)
                    .map(|i| serde_json::to_string(&change(&format!("01FORGEFUZZ{n:05}{i:09}"), paths[(rng.next() % 8) as usize])).unwrap())
                    .collect();
                if rng.next().is_multiple_of(3) {
                    ls.push(String::from_utf8_lossy(&(0..(rng.next() % 200)).map(|_| rng.next() as u8).collect::<Vec<u8>>()).into_owned());
                }
                write(&data, &format!("forge-fuzz-{n:03}"), &ls);
            }
        }
        _ => panic!("usage: forge_batch <repo> valid|hostile|duplicate|fuzz [seed]"),
    }
    println!("forged {kind} batches in {}", data.join("batches").display());
}
