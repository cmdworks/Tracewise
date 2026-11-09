//! `tracewise ignore <pattern>...`: adds lines to `.tracewiseignore`. Past records are kept; `tracewise prune` removes them.
use super::*;

fn refuse(msg: &str) -> ! {
    eprintln!("tracewise: {msg}");
    process::exit(1);
}

/// Appends the new patterns to the file; returns (added, already there). Nothing is written when any pattern is unusable.
fn add_patterns(file: &Path, patterns: &[String]) -> Result<(Vec<String>, Vec<String>), String> {
    let text = fs::read_to_string(file).unwrap_or_default();
    let mut out = text.clone();
    let (mut added, mut already) = (Vec::new(), Vec::new());
    for p in patterns.iter().map(|p| p.trim()) {
        if p.is_empty() || p.contains('\n') || p.starts_with('#') || p.starts_with('!') || matches!(p, "*" | "**" | "/" | "/**" | "**/*") {
            return Err(format!("'{p}' is not a usable ignore pattern (empty, a comment, a negation, or everything)"));
        }
        if text.lines().any(|l| l.trim() == p) || added.iter().any(|a: &String| a == p) {
            already.push(p.to_string());
            continue;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(p);
        out.push('\n');
        added.push(p.to_string());
    }
    if !added.is_empty() {
        fs::write(file, out).map_err(|e| format!("cannot write .tracewiseignore: {e}"))?;
    }
    Ok((added, already))
}

pub(crate) fn run_ignore(json: bool, repo_paths: &RepoPaths, patterns: Vec<String>) {
    // an agent could hide its own edits from the history by ignoring them
    if tracewise_core::model::Actor::from_env().is_some_and(|a| a.kind == "agent") {
        refuse("refused: agents cannot change what Tracewise ignores");
    }
    let (added, already) = match add_patterns(&repo_paths.work_tree.join(".tracewiseignore"), &patterns) {
        Ok(r) => r,
        Err(e) => refuse(&e),
    };
    if json {
        println!("{}", serde_json::json!({ "added": added, "already": already }));
    } else {
        added.iter().for_each(|p| println!("Ignoring {p}"));
        already.iter().for_each(|p| println!("Already ignored: {p}"));
        if !added.is_empty() {
            println!("Records already made stay; `tracewise prune --dry-run` shows what could be removed.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_once_keeps_existing_lines_and_refuses_everything() {
        let d = tempfile::TempDir::new().unwrap();
        let f = d.path().join(".tracewiseignore");
        fs::write(&f, "target/").unwrap();
        let (added, already) = add_patterns(&f, &["dist/".into(), "target/".into(), "dist/".into()]).unwrap();
        assert_eq!((added.len(), already.len()), (1, 2));
        assert_eq!(fs::read_to_string(&f).unwrap(), "target/\ndist/\n");
        assert!(add_patterns(&f, &["**".into()]).is_err());
        assert!(add_patterns(&f, &["!keep".into()]).is_err());
    }
}
