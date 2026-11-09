//! after a power cut a blob can be missing or cut short; `tracewise verify` must say so, and `show` must not present a
//! lost text version as a clean diff.
mod common;
use common::Repo;

fn blobs(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(blobs(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn verify_reports_a_blob_that_did_not_reach_the_disk() {
    let repo = Repo::new("");
    repo.edit_as("human:me", "a.txt", "content that is stored\n");
    assert!(repo.me(&["verify"]).ok(), "a healthy history verifies");
    let lost = blobs(&repo.dir().join(".git/tracewise/objects"));
    assert!(!lost.is_empty(), "the scan stored a blob");
    for b in lost {
        std::fs::remove_file(b).unwrap();
    }
    let out = repo.me(&["verify"]);
    assert!(!out.ok(), "a lost blob is a failed verify: {}", out.out);
}

#[test]
fn show_fails_on_a_missing_text_version_but_not_on_a_binary_file() {
    let repo = Repo::new("");
    repo.write("b.bin", "");
    std::fs::write(repo.dir().join("b.bin"), [0u8, 1, 2, 3]).unwrap();
    let text_id = repo.edit_as("human:me", "a.txt", "one\n");
    let bin_id = repo.rows(&[]).iter().find(|c| c["path"] == "b.bin").unwrap()["id"].as_str().unwrap().to_string();
    assert!(repo.me(&["show", &text_id]).ok());
    assert!(repo.me(&["show", &bin_id]).ok(), "a hash-only file has no stored text and is fine");

    let after = repo.rows(&[]).iter().find(|c| c["id"] == text_id.as_str()).unwrap()["after"].as_str().unwrap().to_string();
    std::fs::remove_file(repo.dir().join(".git/tracewise/objects").join(&after[..2]).join(&after[2..])).unwrap();
    let out = repo.me(&["show", &text_id]);
    assert_eq!(out.code, 1, "{}", out.out);
    assert!(out.err.contains("missing or unreadable"), "{}", out.err);
}
