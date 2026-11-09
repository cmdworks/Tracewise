//! with `encrypt = true` the remote holds no readable record or file content; another device needs the key.
mod common;
use common::Repo;
use std::process::Command;

const CANARY: &str = "CANARY-9f3c1d-contents";

fn tracewise(repo: &Repo, args: &[&str]) -> common::Run {
    repo.with("human:me", args, None, &[("TRACEWISE_PROFILE", "full")])
}

fn git_out(remote: &std::path::Path, args: &[&str]) -> String {
    let out = Command::new("git").arg(format!("--git-dir={}", remote.display())).args(args).output().unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Everything readable in every ref of the remote: file names and file contents.
fn remote_text(remote: &std::path::Path) -> String {
    let mut all = String::new();
    for r in git_out(remote, &["for-each-ref", "--format=%(refname)"]).lines() {
        let tree = git_out(remote, &["ls-tree", "-r", r]);
        all.push_str(&tree);
        for line in tree.lines() {
            let sha = line.split_whitespace().nth(2).unwrap_or("");
            all.push_str(&git_out(remote, &["cat-file", "-p", sha]));
        }
    }
    all
}

fn device(url: &str, init_args: &[&str]) -> Repo {
    let repo = Repo::uninitialised("[profile]\nshare = true\n");
    repo.write("a.txt", "base\n");
    assert!(tracewise(&repo, &["init"]).ok());
    let mut args = vec!["share", "init", "--remote", url, "--level", "full", "--hidden"];
    args.extend_from_slice(init_args);
    let r = tracewise(&repo, &args);
    assert!(r.ok(), "{}", r.err);
    repo
}

#[test]
fn an_encrypted_push_hides_everything_and_another_device_needs_the_key() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());

    let first = device(&url, &["--encrypt"]);
    assert!(first.read(".tracewiseconfig").contains("encrypt = true"));
    first.write("a.txt", &format!("base\n{CANARY}\n"));
    assert!(tracewise(&first, &["scan"]).ok());
    let pushed = tracewise(&first, &["push"]);
    assert!(pushed.ok(), "{}", pushed.err);

    let text = remote_text(&remote);
    assert!(text.contains("batch.json") && text.contains("\"encrypted\": true"), "the batch says it is encrypted");
    assert!(!text.contains(CANARY) && !text.contains("a.txt"), "no file name or content on the remote");

    // a second device without the key is refused, and nothing is imported
    let second = device(&url, &[]);
    let refused = tracewise(&second, &["fetch"]);
    assert!(!refused.ok() && refused.err.contains("encrypted") && refused.err.contains("import the key"), "{}", refused.err);
    let before = tracewise(&second, &["log", "--json"]).json().as_array().unwrap().len();

    // with the key it reads the history
    std::fs::copy(first.dir().join(".git/tracewise/share-keys.json"), second.dir().join(".git/tracewise/share-keys.json")).unwrap();
    let fetched = tracewise(&second, &["fetch"]);
    assert!(fetched.ok(), "{}", fetched.err);
    let after = tracewise(&second, &["log", "--json"]).json().as_array().unwrap().len();
    assert!(after > before, "records arrived: {before} -> {after}");
    assert!(tracewise(&second, &["verify"]).ok());
}

#[test]
fn a_push_with_encryption_on_and_no_key_is_refused_not_sent_in_the_clear() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());
    let repo = device(&url, &[]);
    repo.write(".tracewiseconfig", &format!("[profile]\nshare = true\n\n[share]\nremote = \"{url}\"\nlevel = \"full\"\nencrypt = true\n"));
    repo.write("a.txt", &format!("base\n{CANARY}\n"));
    assert!(tracewise(&repo, &["scan"]).ok());
    let out = tracewise(&repo, &["push"]);
    assert!(!out.ok() && out.err.contains("has no key"), "{}", out.err);
    assert!(!remote_text(&remote).contains(CANARY));
}

fn push_edit(repo: &Repo, text: &str) {
    repo.write("a.txt", text);
    assert!(tracewise(repo, &["scan"]).ok());
    let pushed = tracewise(repo, &["push"]);
    assert!(pushed.ok(), "{}", pushed.err);
}

fn exported(repo: &Repo) -> String {
    let out = tracewise(repo, &["share", "key", "export"]);
    assert!(out.ok(), "{}", out.err);
    out.out.trim().to_string()
}

#[test]
fn keys_are_handed_over_rotated_and_an_agent_cannot_touch_them() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());
    let first = device(&url, &["--encrypt"]);
    push_edit(&first, "one\n");

    let listed = tracewise(&first, &["share", "key", "list", "--json"]).json();
    assert_eq!(listed["keys"].as_array().unwrap().len(), 1);
    let key = exported(&first);
    assert!(
        key.starts_with("tracewisekey1:") && !listed.to_string().contains(&key[key.rfind(':').unwrap() + 1..]),
        "list never shows the key"
    );
    for args in [&["share", "key", "export"][..], &["share", "key", "rotate"], &["share", "key", "import", &key]] {
        let r = first.with("agent:claude:s1", args, None, &[("TRACEWISE_PROFILE", "full")]);
        assert!(!r.ok() && r.err.contains("agents cannot"), "{args:?}: {}", r.err);
    }

    // the second device imports the key by hand-over and reads the history
    let second = device(&url, &[]);
    assert!(tracewise(&second, &["share", "key", "import", &key]).ok());
    assert!(tracewise(&second, &["fetch"]).ok());

    // a rotation: a new batch is under the new key; the second device cannot read it until it imports that key as current
    assert!(tracewise(&first, &["share", "key", "rotate"]).ok());
    push_edit(&first, "one\ntwo\n");
    let blocked = tracewise(&second, &["fetch"]);
    assert!(!blocked.ok() && blocked.err.contains("import the key"), "{}", blocked.err);
    assert!(tracewise(&second, &["share", "key", "import", "--current", &exported(&first)]).ok());
    assert!(tracewise(&second, &["fetch"]).ok());
    assert_eq!(tracewise(&second, &["share", "key", "list", "--json"]).json()["keys"].as_array().unwrap().len(), 2);
}

#[test]
fn rekey_recovers_a_lost_key_from_the_local_history_and_prune_needs_yes() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());
    let first = device(&url, &["--encrypt"]);
    push_edit(&first, &format!("one\n{CANARY}\n"));
    let second = device(&url, &[]);
    assert!(tracewise(&second, &["share", "key", "import", &exported(&first)]).ok());
    assert!(tracewise(&second, &["fetch"]).ok());

    // the key is lost on the first device; its own database is intact
    std::fs::remove_file(first.dir().join(".git/tracewise/share-keys.json")).unwrap();
    let no_yes = tracewise(&first, &["share", "key", "rekey", "--prune-old"]);
    assert!(!no_yes.ok() && no_yes.err.contains("--yes"), "{}", no_yes.err);
    let done = tracewise(&first, &["share", "key", "rekey", "--prune-old", "--yes", "--json"]);
    assert!(done.ok(), "{}", done.err);
    assert!(done.json()["records"].as_u64().unwrap() >= 1, "{}", done.out);
    assert!(!remote_text(&remote).contains(CANARY));
    assert!(tracewise(&first, &["fetch"]).ok(), "the unreadable old batch does not block this device: it is noted as imported");

    // the other device takes the new key and still has everything
    let before = tracewise(&second, &["log", "--json"]).json().as_array().unwrap().len();
    assert!(tracewise(&second, &["share", "key", "import", "--current", &exported(&first)]).ok());
    let fetched = tracewise(&second, &["fetch"]);
    assert!(fetched.ok(), "{}", fetched.err);
    assert_eq!(tracewise(&second, &["log", "--json"]).json().as_array().unwrap().len(), before, "the same records, none doubled");
    assert!(tracewise(&second, &["verify"]).ok());
}

#[test]
fn doctor_warns_about_an_unexported_key_a_missing_key_and_full_on_the_projects_own_remote() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("project.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());
    let warnings = |repo: &Repo| -> String { tracewise(repo, &["doctor", "--json"]).json()["sharing_warnings"].to_string() };

    // level full, no encryption, on the project's own origin
    let repo = Repo::uninitialised("[profile]\nshare = true\n");
    repo.write("a.txt", "x\n");
    assert!(tracewise(&repo, &["init"]).ok());
    assert!(Command::new("git").current_dir(repo.dir()).args(["remote", "add", "origin", &url]).status().unwrap().success());
    assert_eq!(warnings(&repo), "[]", "sharing not set up: nothing to say");
    assert!(tracewise(&repo, &["share", "init", "--level", "full"]).ok());
    assert!(warnings(&repo).contains("level full"), "{}", warnings(&repo));

    // encryption on: the key was made but never exported
    assert!(tracewise(&repo, &["share", "init", "--level", "summary", "--encrypt"]).ok());
    assert!(warnings(&repo).contains("never exported"), "{}", warnings(&repo));
    assert!(tracewise(&repo, &["share", "key", "export"]).ok());
    assert_eq!(warnings(&repo), "[]", "after the export nothing is left to warn about");

    // encryption on, key gone
    std::fs::remove_file(repo.dir().join(".git/tracewise/share-keys.json")).unwrap();
    assert!(warnings(&repo).contains("has no key"), "{}", warnings(&repo));
}

#[test]
fn share_status_reports_the_setup_and_never_the_key() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());
    let repo = Repo::uninitialised("[profile]\nshare = true\n");
    repo.write("a.txt", "x\n");
    assert!(tracewise(&repo, &["init"]).ok());
    assert_eq!(tracewise(&repo, &["share", "status", "--json"]).json()["configured"], false);
    assert!(tracewise(&repo, &["share", "init", "--remote", &url, "--level", "summary", "--hidden", "--encrypt"]).ok());
    let out = tracewise(&repo, &["share", "status", "--json"]);
    let v = out.json();
    assert_eq!(
        (v["configured"].clone(), v["level"].as_str(), v["refs"].as_str(), v["encrypt"].clone(), v["store"].as_str()),
        (true.into(), Some("summary"), Some("hidden"), true.into(), Some("url")),
        "{v}"
    );
    assert!(v["key_id"].as_str().is_some() && v["trusted"] == true && v["key_exported"] == false);
    let exported = tracewise(&repo, &["share", "key", "export"]).out;
    let secret = exported.trim().rsplit(':').next().unwrap().to_string();
    assert!(!out.out.contains(&secret), "status never prints the key");
    assert_eq!(tracewise(&repo, &["share", "status", "--json"]).json()["key_exported"], true);
    // an agent may read it
    assert!(repo.with("agent:claude:s1", &["share", "status", "--json"], None, &[("TRACEWISE_PROFILE", "full")]).ok());
}

/// S8: all four combinations of store and encryption, two devices each: what one pushes the other reads, and an encrypted remote holds no text.
#[test]
fn every_combination_of_store_and_encryption_works_between_two_devices() {
    for (own_remote, encrypt) in [(true, false), (true, true), (false, false), (false, true)] {
        let d = tempfile::tempdir().unwrap();
        // the project's own remote, with code on a branch (store = origin), or a separate companion repository
        let project = d.path().join("project.git");
        let companion = d.path().join("companion.git");
        for r in [&project, &companion] {
            assert!(Command::new("git").args(["init", "-q", "--bare", r.to_str().unwrap()]).status().unwrap().success());
        }
        let shared = if own_remote { &project } else { &companion };
        let url = format!("file://{}", shared.display());

        let mk = |first: bool| {
            let repo = Repo::uninitialised("[profile]\nshare = true\n");
            repo.write("a.txt", "base\n");
            assert!(tracewise(&repo, &["init"]).ok());
            if own_remote {
                assert!(Command::new("git").current_dir(repo.dir()).args(["remote", "add", "origin", &url]).status().unwrap().success());
            }
            let mut args = vec!["share", "init", "--level", "full"];
            if !own_remote {
                args.extend(["--remote", &url, "--hidden"]);
            }
            if first && encrypt {
                args.push("--encrypt");
            }
            let r = tracewise(&repo, &args);
            assert!(r.ok(), "{own_remote} {encrypt}: {}", r.err);
            repo
        };
        let (a, b) = (mk(true), mk(false));
        push_edit(&a, &format!("base\n{CANARY}\n"));

        let text = remote_text(shared);
        assert_eq!(
            text.contains(CANARY),
            !encrypt,
            "own={own_remote} encrypt={encrypt}: the canary is visible exactly when it is not encrypted"
        );
        if encrypt {
            // the second device needs the key and the setting
            b.write(".tracewiseconfig", &format!("{}encrypt = true\n", b.read(".tracewiseconfig")));
            assert!(!tracewise(&b, &["fetch"]).ok(), "no key yet");
            assert!(tracewise(&b, &["share", "key", "import", &exported(&a)]).ok());
        }
        let got = tracewise(&b, &["fetch"]);
        assert!(got.ok(), "own={own_remote} encrypt={encrypt}: {}", got.err);
        let paths: Vec<String> = tracewise(&b, &["log", "--json"])
            .json()
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["path"].as_str().unwrap_or("").to_string())
            .collect();
        assert!(paths.iter().any(|p| p == "a.txt"), "{own_remote} {encrypt}: b has a's change of a.txt: {paths:?}");
        assert!(tracewise(&b, &["verify"]).ok());
    }
}

#[test]
fn a_changed_encrypted_batch_fails_its_signature_before_any_decryption() {
    let d = tempfile::tempdir().unwrap();
    let remote = d.path().join("meta.git");
    assert!(Command::new("git").args(["init", "-q", "--bare", remote.to_str().unwrap()]).status().unwrap().success());
    let url = format!("file://{}", remote.display());
    let first = device(&url, &["--encrypt"]);
    push_edit(&first, "one\n");
    let second = device(&url, &[]);
    assert!(tracewise(&second, &["share", "key", "import", &exported(&first)]).ok());
    assert!(tracewise(&second, &["fetch"]).ok());
    // the first device's own copy of the batch is altered, then rebuilt (rebuild reads every batch)
    let batches = first.dir().join(".git/tracewise/batches");
    let batch = std::fs::read_dir(&batches).unwrap().flatten().find(|e| e.path().is_dir()).unwrap().path();
    let f = batch.join("changes.jsonl.zst");
    let mut bytes = std::fs::read(&f).unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    std::fs::write(&f, bytes).unwrap();
    let out = tracewise(&first, &["rebuild"]);
    assert!(!out.ok() && out.err.contains("signature"), "{}", out.err);
}
