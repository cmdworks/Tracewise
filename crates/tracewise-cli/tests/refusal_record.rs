//! A coordination refusal (an edit of a file another agent holds) leaves a `refused` command, so it can be seen in the history.
mod common;
use common::Repo;

const COORD: &str = "[profile]\ncoordination = true\n";

fn pre(repo: &Repo, agent: &str, session: &str) -> String {
    let payload = format!(r#"{{"agent":"{agent}","session":"{session}","paths":["a.txt"]}}"#);
    repo.with(&format!("agent:{agent}:{session}"), &["hook", "pre-edit"], Some(&payload), &[]).out
}

fn commands(repo: &Repo) -> Vec<serde_json::Value> {
    serde_json::from_str(&repo.me(&["commands", "--json"]).out).unwrap_or_default()
}

#[test]
fn a_refused_edit_is_recorded_with_who_and_why() {
    let repo = Repo::new(COORD);
    repo.edit_as("agent:codex:s0", "a.txt", "one\n");
    repo.later();
    let intent = repo.with(
        "agent:claude:s1",
        &["intent", "add", "--why", "change a.txt so the lock test has something to hold", "a.txt"],
        None,
        &[],
    );
    assert!(intent.ok(), "{} {}", intent.out, intent.err);
    assert!(pre(&repo, "claude", "s1").contains("\"allow\""), "claude takes the lock");
    let denied = pre(&repo, "codex", "s2");
    assert!(denied.contains("\"deny\"") && denied.contains("locked by claude"), "{denied}");
    let refused: Vec<_> = commands(&repo).into_iter().filter(|c| c["kind"] == "refused").collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0]["text"].as_str().unwrap().contains("a.txt") && refused[0]["text"].as_str().unwrap().contains("locked by claude"));
    assert_eq!(refused[0]["actor"]["name"], "codex");
    // the allowed edit left no refusal
    assert!(pre(&repo, "claude", "s1").contains("\"allow\""));
    assert_eq!(commands(&repo).iter().filter(|c| c["kind"] == "refused").count(), 1);
}
