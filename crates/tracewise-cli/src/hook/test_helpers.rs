use super::*;

#[test]
pub fn trail_files_and_hook_configs_are_protected() {
    let wt = Path::new("/w/repo");
    for p in [
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".agents/hooks.json",
        ".codex/hooks.json",
        ".gemini/settings.json",
        ".cursor/hooks.json",
        ".git/trail/trail.db",
        ".git/trail",
        ".trailconfig",
        "/w/repo/.git/trail/objects",
        "src/../.claude/settings.json",
    ] {
        assert!(is_trail_protected(p, wt), "{p}");
    }
    for p in ["src/main.rs", ".claude/skills/x.md", ".gitignore", "/etc/hosts", "../other/.claude/settings.json"] {
        assert!(!is_trail_protected(p, wt), "{p}");
    }
}
