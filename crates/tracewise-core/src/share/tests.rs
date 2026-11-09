use super::batch::git_cmd;
use super::config::{owner_prefix, TRUSTED_REMOTES_FILE};
use super::*;
use std::fs;
use std::io;

#[test]
fn test_tracewiseconfig_read_write() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    assert!(read_tracewiseconfig(root).is_none());

    write_tracewiseconfig(root, "file:///tmp/meta.git", "full").unwrap();
    let cfg = read_tracewiseconfig(root).unwrap();
    assert_eq!(cfg.remote, "file:///tmp/meta.git");
    assert_eq!(cfg.level, "full");

    // Test inline comments and quoted values containing '='
    let custom_cfg = "[share] # companion remote configuration\n\
                      remote = \"https://github.com/org/repo.git?branch=main\" # remote URL\n\
                      level = \"metadata\" # share level\n";
    fs::write(root.join(".tracewiseconfig"), custom_cfg).unwrap();
    let parsed = read_tracewiseconfig(root).unwrap();
    assert_eq!(parsed.remote, "https://github.com/org/repo.git?branch=main");
    assert_eq!(parsed.level, "metadata");
}

#[test]
fn test_validate_remote_url() {
    // Valid URLs
    assert!(validate_remote_url("https://github.com/hasan-py/Hayroo.git").is_ok());
    assert!(validate_remote_url("ssh://git@github.com/hasan-py/Hayroo.git").is_ok());
    assert!(validate_remote_url("git@github.com:hasan-py/Hayroo.git").is_ok());
    assert!(validate_remote_url("file:///tmp/meta.git").is_ok());
    assert!(validate_remote_url("/path/to/bare/repo.git").is_ok());

    // Invalid URLs (injection vectors and unauthorized schemes)
    assert!(validate_remote_url("--upload-pack=evil").is_err());
    assert!(validate_remote_url("-u").is_err());
    assert!(validate_remote_url("http://evil.com/repo.git").is_err());
    assert!(validate_remote_url("ftp://foo.com/repo.git").is_err());
    assert!(validate_remote_url("ext::sh -c evil%").is_err());
    assert!(validate_remote_url("relative/path/repo.git").is_err());
    assert!(validate_remote_url("").is_err());
    assert!(validate_remote_url("git@-evil:path").is_err());
    assert!(validate_remote_url("https://foo.com/repo\nevil").is_err());

    // Malicious .tracewiseconfig should be rejected by read_tracewiseconfig
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(root.join(".tracewiseconfig"), "[share]\nremote = \"--upload-pack=evil\"\nlevel = \"full\"\n").unwrap();
    assert!(read_tracewiseconfig(root).is_none());
}

#[test]
fn test_push_requires_approved_remote() {
    let repo = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    write_tracewiseconfig(repo.path(), "file:///tmp/evil.git", "full").unwrap();

    let err = push(repo.path(), data.path()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(err.to_string().contains("tracewise share trust"));

    // approving a different remote does not help
    trust_remote(data.path(), "file:///tmp/other.git").unwrap();
    assert!(push(repo.path(), data.path()).is_err());

    // approving the right one passes the gate (nothing recorded yet, so nothing to push)
    trust_remote(data.path(), "file:///tmp/evil.git").unwrap();
    assert!(matches!(push(repo.path(), data.path()), Ok(None)));
}

#[test]
fn test_trust_remote_is_idempotent_and_validated() {
    let data = tempfile::tempdir().unwrap();
    assert!(!is_remote_trusted(data.path(), "https://example.com/a.git"));
    trust_remote(data.path(), "https://example.com/a.git").unwrap();
    trust_remote(data.path(), "https://example.com/a.git").unwrap();
    assert!(is_remote_trusted(data.path(), "https://example.com/a.git"));
    let stored = fs::read_to_string(data.path().join(TRUSTED_REMOTES_FILE)).unwrap();
    assert_eq!(stored.lines().count(), 1);
    assert!(trust_remote(data.path(), "--upload-pack=evil").is_err());
    assert!(!is_remote_trusted(data.path(), "--upload-pack=evil"));
}

#[test]
fn test_contains_secret() {
    assert!(contains_secret(b"x\n-----BEGIN OPENSSH PRIVATE KEY-----\nabc"));
    assert!(contains_secret(b"key = AKIAIOSFODNN7EXAMPLE"));
    assert!(contains_secret(b"token: ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
    assert!(contains_secret(b"STRIPE=sk_live_abcdefghijklmnopqrstu"));
    // ordinary text and short look-alikes pass
    assert!(!contains_secret(b"fn main() { println!(\"hello\"); }"));
    assert!(!contains_secret(b"AKIA is a prefix, ghp_short, -----BEGIN CERTIFICATE-----"));
    assert!(!contains_secret(&[0xff, 0xfe, 0x00]));
}

#[test]
fn test_is_secret_path() {
    assert!(is_secret_path(".env"));
    assert!(is_secret_path(".env.local"));
    assert!(is_secret_path(".env.production"));
    assert!(is_secret_path("config/.env"));
    assert!(is_secret_path("id_rsa"));
    assert!(is_secret_path("id_ed25519"));
    assert!(is_secret_path(".ssh/id_rsa.pub"));
    assert!(is_secret_path("server.pem"));
    assert!(is_secret_path("certs/private.key"));
    assert!(is_secret_path("cert.pfx"));
    assert!(is_secret_path("cert.p12"));

    assert!(!is_secret_path("src/main.rs"));
    assert!(!is_secret_path("pay.txt"));
    assert!(!is_secret_path("README.md"));
    assert!(!is_secret_path("package.json"));
}

#[test]
fn test_ensure_companion_repo_invalid() {
    let temp = tempfile::tempdir().unwrap();
    let remote_dir = temp.path().join("remote");
    // Nonexistent local repo should fail clone with error
    let res = ensure_companion_repo("file:///nonexistent/repo.git", &remote_dir);
    assert!(res.is_err());
}

#[test]
fn test_safe_names_from_a_remote() {
    assert!(is_safe_id("01JBZ6Q4K7X8V2N3M4P5Q6R7S8"));
    assert!(is_safe_id("batch-1_a"));
    for bad in ["", "..", "../x", "a/b", "a b", "a\\b", ".git"] {
        assert!(!is_safe_id(bad), "{bad}");
    }
    assert!(is_safe_rel_path("changes.jsonl.zst"));
    assert!(is_safe_rel_path("blobs/ab12"));
    assert!(is_safe_rel_path("src/main.rs"));
    for bad in ["", "/etc/passwd", "../x", "a/../../x", "a/..", ".git/hooks/pre-commit", "x/.GIT/config", "a\0b"] {
        assert!(!is_safe_rel_path(bad), "{bad}");
    }
}

#[test]
fn test_owner_prefix_and_same_place_rule() {
    assert_eq!(owner_prefix("git@github.com:theasmat/app.git").as_deref(), Some("git@github.com:theasmat/"));
    assert_eq!(owner_prefix("https://github.com/TheAsmat/app.git").as_deref(), Some("https://github.com/theasmat/"));
    assert_eq!(owner_prefix("file:///tmp/a/project.git").as_deref(), Some("file:///tmp/a/"));
    assert_eq!(owner_prefix("/tmp/a/project.git").as_deref(), Some("/tmp/a/"));
    // no owner part to compare
    assert_eq!(owner_prefix("https://github.com/app.git"), None);
    assert_eq!(owner_prefix("git@github.com:app.git"), None);
    // another owner or host is a different place
    assert_ne!(owner_prefix("git@github.com:evil/x.git"), owner_prefix("git@github.com:theasmat/x.git"));
    assert_ne!(owner_prefix("git@evil.com:theasmat/x.git"), owner_prefix("git@github.com:theasmat/x.git"));
}

#[test]
fn test_fetch_refuses_an_unapproved_remote_from_another_place() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    let g = |args: &[&str]| {
        assert!(git_cmd().arg("-C").arg(&repo).args(args).output().unwrap().status.success());
    };
    g(&["init", "-q"]);
    g(&["remote", "add", "origin", "git@github.com:theasmat/app.git"]);
    write_tracewiseconfig(&repo, "git@evil.example:attacker/meta.git", "full").unwrap();
    let data = repo.join(".git/tracewise");
    fs::create_dir_all(&data).unwrap();
    let err = fetch(&repo, &data).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(!data.join("remote").exists(), "nothing was cloned from the unapproved remote");
    // approving it here makes the check pass (the clone itself then fails: no such host)
    trust_remote(&data, "git@evil.example:attacker/meta.git").unwrap();
    assert!(may_sync_from(&repo, &data, "git@evil.example:attacker/meta.git"));
    // a companion next to the origin is allowed without approval
    assert!(may_sync_from(&repo, &data, "git@github.com:theasmat/meta.git"));
}
