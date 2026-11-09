//! Parity corpus: run the real tool, see which paths it changed, and require the shim's `detect_write_targets` to name
//! every one of them (or say `AnyInCwd`). A path the shim misses is a write that no lock can see.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracewise_shim::{detect_write_targets, WriteTargets};

fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            let meta = std::fs::symlink_metadata(&p).unwrap();
            let rel = p.strip_prefix(root).unwrap().to_path_buf();
            let mut state = format!("{:o} {}", std::os::unix::fs::PermissionsExt::mode(&meta.permissions()), meta.len());
            if meta.is_dir() {
                stack.push(p);
            } else if meta.is_symlink() {
                state += &format!(" -> {:?}", std::fs::read_link(&p).unwrap());
            } else {
                state += &format!(" {:?}", std::fs::read(&p).unwrap());
            }
            out.insert(rel, state);
        }
    }
    out
}

/// Runs `tool args` in a fresh directory holding `a.txt`, `b.txt`, `dir/c.txt`; returns the changed paths and the shim's answer.
fn run(tool: &str, args: &[&str], stdin: &str) -> (Vec<PathBuf>, WriteTargets, bool) {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap();
    std::fs::write(root.join("a.txt"), "alpha\n").unwrap();
    std::fs::write(root.join("b.txt"), "beta\n").unwrap();
    std::fs::create_dir(root.join("dir")).unwrap();
    std::fs::write(root.join("dir/c.txt"), "gamma\n").unwrap();
    let before = snapshot(&root);
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let mut child = Command::new(tracewise_shim::find_real_tool(tool, None))
        .args(&args)
        .current_dir(&root)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), stdin.as_bytes()).unwrap();
    let ok = child.wait().unwrap().success();
    let after = snapshot(&root);
    let mut changed: Vec<PathBuf> = after.keys().chain(before.keys()).filter(|k| before.get(*k) != after.get(*k)).cloned().collect();
    changed.sort();
    changed.dedup();
    (changed, detect_write_targets(tool, &args), ok)
}

fn covered(changed: &[PathBuf], targets: &WriteTargets) -> Result<(), String> {
    match targets {
        WriteTargets::AnyInCwd => Ok(()),
        WriteTargets::None if changed.is_empty() => Ok(()),
        WriteTargets::None => Err(format!("called read-only, but changed {changed:?}")),
        WriteTargets::Paths(ps) => {
            let missed: Vec<_> = changed
                .iter()
                .filter(|c| !ps.iter().any(|p| p == *c || c.starts_with(p) || p.starts_with(c) && !p.as_os_str().is_empty()))
                .collect();
            if missed.is_empty() {
                Ok(())
            } else {
                Err(format!("changed {missed:?}, shim named {ps:?}"))
            }
        }
    }
}

#[test]
fn shim_names_every_path_the_real_tool_changes() {
    let cases: &[(&str, &[&str], &str)] = &[
        ("rm", &["a.txt"], ""),
        ("rm", &["-f", "a.txt", "b.txt"], ""),
        ("rm", &["-r", "dir"], ""),
        ("rm", &["--", "a.txt"], ""),
        ("rm", &["-rf", "dir", "a.txt"], ""),
        ("mv", &["a.txt", "n.txt"], ""),
        ("mv", &["a.txt", "b.txt", "dir"], ""),
        ("mv", &["-f", "a.txt", "dir/"], ""),
        ("mv", &["-n", "a.txt", "b.txt"], ""),
        ("mv", &["dir", "d2"], ""),
        ("cp", &["a.txt", "n.txt"], ""),
        ("cp", &["a.txt", "b.txt", "dir"], ""),
        ("cp", &["-R", "dir", "d2"], ""),
        ("cp", &["-p", "a.txt", "b.txt"], ""),
        ("cp", &["-n", "a.txt", "b.txt"], ""),
        ("tee", &["n.txt"], "hi\n"),
        ("tee", &["-a", "a.txt", "b.txt"], "hi\n"),
        ("tee", &["--", "-weird"], "hi\n"),
        ("tee", &[], "hi\n"),
        ("chflags", &["hidden", "a.txt"], ""),
        ("chflags", &["-R", "hidden", "dir"], ""),
        ("chflags", &["-h", "nohidden", "a.txt", "b.txt"], ""),
        ("xattr", &["-w", "k", "v", "a.txt"], ""),
        ("xattr", &["-w", "-r", "k", "v", "dir"], ""),
        ("xattr", &["-d", "k", "a.txt"], ""),
        ("xattr", &["-c", "a.txt", "b.txt"], ""),
        ("xattr", &["-l", "a.txt"], ""),
        ("xattr", &["-p", "k", "a.txt"], ""),
        ("xattr", &["a.txt"], ""),
        ("sed", &["-i", "", "s/alpha/x/", "a.txt"], ""),
        ("sed", &["-i.bak", "s/alpha/x/", "a.txt"], ""),
        ("sed", &["-i", "-e", "s/alpha/x/", "a.txt", "b.txt"], ""),
        ("sed", &["-Ei", "", "s/(a)/x/", "a.txt"], ""),
        ("sed", &["-n", "p", "a.txt"], ""),
        ("sed", &["s/alpha/x/", "a.txt"], ""),
        ("sed", &["-i", "", "-n", "1p", "a.txt"], ""),
        ("sed", &["-i", "", "w out.txt", "a.txt"], ""),
        ("sed", &["-i", "", "s/alpha/x/w out.txt", "a.txt"], ""),
        ("sed", &["-n", "s/alpha/x/w out.txt", "a.txt"], ""),
        ("rm", &["-rv", "dir"], ""),
        ("rm", &["-d", "dir/c.txt"], ""),
        ("mv", &["-v", "-i", "a.txt", "n.txt"], ""),
        ("cp", &["-a", "a.txt", "n.txt"], ""),
        ("cp", &["-c", "a.txt", "n.txt"], ""),
        ("xattr", &["-wr", "k", "v", "dir"], ""),
        ("xattr", &["-dr", "k", "dir"], ""),
        ("xattr", &["-cr", "dir"], ""),
        ("xattr", &["-s", "-w", "k", "v", "a.txt"], ""),
        ("xattr", &["-w", "k", "-v", "a.txt"], ""),
        ("chflags", &["-fv", "hidden", "a.txt"], ""),
        ("chflags", &["0", "a.txt"], ""),
        ("sed", &["-i", "", "-e", "s/alpha/x/", "a.txt"], ""),
        ("sed", &["-i", "", "-e", "s/alpha/x/", "-e", "w out.txt", "a.txt"], ""),
        ("sed", &["-i", "", "/alpha/w out.txt", "a.txt"], ""),
        ("sed", &["-i", "", "1,2w out.txt", "a.txt"], ""),
        ("sed", &["-i", "", "$!N;w out.txt", "a.txt"], ""),
        ("sed", &["-i", "", "s/alpha/x/;w out.txt", "a.txt"], ""),
        ("sed", &["-i", "", "s|alpha|x|w out.txt", "a.txt"], ""),
        ("sed", &["-n", "/alpha/{w out.txt\n}", "a.txt"], ""),
    ];
    let mut bad = Vec::new();
    for (tool, args, stdin) in cases {
        let (changed, targets, _) = run(tool, args, stdin);
        if let Err(e) = covered(&changed, &targets) {
            bad.push(format!("{tool} {args:?}: {e}"));
        }
    }
    assert!(bad.is_empty(), "shim and real tool disagree:\n{}", bad.join("\n"));
}

#[test]
fn non_utf8_argument_reaches_the_real_tool_unchanged() {
    use std::os::unix::ffi::OsStrExt;
    let d = tempfile::tempdir().unwrap();
    let shim = d.path().join("sed");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_tracewise-shim"), &shim).unwrap();
    let f = d.path().join("f");
    std::fs::write(&f, "x\n").unwrap();
    let script = std::ffi::OsStr::from_bytes(b"s/x/\xff/");
    let status = Command::new(&shim).arg("-i").arg("").arg(script).arg(&f).env("PATH", "/usr/bin:/bin").status().unwrap();
    assert!(status.success(), "the shim crashed on a non-UTF-8 argument: {status:?}");
    assert_eq!(std::fs::read(&f).unwrap(), b"\xff\n");
}
