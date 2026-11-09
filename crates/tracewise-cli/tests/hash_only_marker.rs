//! `log` and `show` say when a change has no stored version, and `[storage] secrets = "store"` turns the secret rule off.
mod common;
use common::Repo;

fn create<'a>(rows: &'a [serde_json::Value], path: &str) -> &'a serde_json::Value {
    rows.iter().find(|c| c["path"] == path && c["op"] == "create").unwrap_or_else(|| panic!("no create for {path}"))
}

fn key() -> String {
    format!("ghp_{}", "A1b2C3d4E5".repeat(4))
}

#[test]
fn log_and_show_mark_changes_that_have_no_stored_version() {
    let repo = Repo::new("");
    repo.write("plain.txt", "hello\n");
    repo.write("cfg.txt", &format!("token = {}\n", key()));
    std::fs::write(repo.dir().join("pic.bin"), [0u8, 1, 2, 3]).unwrap();
    assert!(repo.me(&["scan"]).ok());

    let all = repo.rows(&[]);
    assert_eq!(create(&all, "plain.txt")["hash_only"], false);
    assert_eq!(create(&all, "cfg.txt")["hash_only"], true);
    assert_eq!(create(&all, "pic.bin")["hash_only"], true);

    let text = repo.me(&["log"]).out;
    assert!(text.lines().any(|l| l.contains("cfg.txt") && l.contains("(hash only)")), "{text}");
    assert!(!text.lines().any(|l| l.contains("plain.txt") && l.contains("(hash only)")), "{text}");

    let id = create(&all, "cfg.txt")["id"].as_str().unwrap().to_string();
    let shown = repo.me(&["show", &id]).out;
    assert!(shown.contains("recorded by hash only"), "{shown}");
    assert!(!shown.contains(&key()), "the key is never printed");
}

#[test]
fn the_secrets_setting_store_keeps_the_file_like_any_text() {
    let repo = Repo::new("[storage]\nsecrets = \"store\"\n");
    repo.write("cfg.txt", &format!("token = {}\n", key()));
    assert!(repo.me(&["scan"]).ok());
    let all = repo.rows(&[]);
    let c = create(&all, "cfg.txt");
    assert_eq!(c["hash_only"], false);
    let shown = repo.me(&["show", c["id"].as_str().unwrap()]).out;
    assert!(shown.contains("ghp_"), "with `store`, the content is kept and shown: {shown}");
}
