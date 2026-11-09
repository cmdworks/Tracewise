//! B1/B2: `doctor` reports missing hook events per agent and how well recent changes are attributed.
mod common;
use common::Repo;

#[test]
fn doctor_reports_attribution_and_partial_hooks() {
    let repo = Repo::uninitialised("");
    repo.write(".cursor/hooks.json", r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"trail hook PreToolUse --from cursor"}]}]}}"#);
    assert!(repo.me(&["init"]).ok());
    repo.edit_as("human:me", "a.txt", "one\n");
    repo.edit_as("human:me", "b.txt", "two\n");

    let v = repo.me(&["doctor", "--json"]).json();
    let a = &v["attribution"];
    assert!(a["changes"].as_u64().unwrap() >= 2, "{a}");
    let by_confidence: u64 = a["confidence"].as_object().unwrap().values().map(|n| n.as_u64().unwrap()).sum();
    assert_eq!(by_confidence, a["changes"].as_u64().unwrap(), "every change has one confidence");
    assert_eq!(v["coverage"]["agents"]["cursor"], true);
    assert_eq!(v["coverage"]["missing_events"]["cursor"], serde_json::json!(["PostToolUse", "Stop", "SessionEnd"]));

    let text = repo.me(&["doctor"]).out;
    assert!(text.contains("attribution (last 7 days):"), "{text}");
    assert!(text.contains("missing hook events: PostToolUse, Stop, SessionEnd"), "{text}");
}
