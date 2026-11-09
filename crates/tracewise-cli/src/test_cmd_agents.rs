use super::*;
use tempfile::tempdir;

#[test]
fn test_agent_install_uninstall_and_idempotence() {
    let tmp = tempdir().unwrap();
    let work_tree = tmp.path().to_path_buf();
    let git_dir = work_tree.join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    let repo_paths = RepoPaths { work_tree: work_tree.clone(), git_dir: git_dir.clone(), git_common_dir: git_dir.clone() };

    // Existing settings with user permissions and custom hook
    let claude_cfg = work_tree.join(".claude/settings.json");
    fs::create_dir_all(claude_cfg.parent().unwrap()).unwrap();
    let initial_settings = serde_json::json!({
        "permissions": {
            "allow": ["Bash(ls:*)"]
        },
        "hooks": {
            "PreToolUse": [
                {
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "echo mine"}]
                }
            ]
        }
    });
    fs::write(&claude_cfg, serde_json::to_string(&initial_settings).unwrap()).unwrap();

    let agents_md = work_tree.join("AGENTS.md");
    fs::write(&agents_md, "# My Project Rules\n\nKeep this line.\n").unwrap();

    // 1. Install claude
    handle_agent_install(&repo_paths, "claude", "project", false).unwrap();

    let installed_settings = fs::read_to_string(&claude_cfg).unwrap();
    assert!(installed_settings.contains("echo mine"));
    assert!(installed_settings.contains("tracewise hook"));
    assert!(installed_settings.contains("--from claude"));

    let skill_file = work_tree.join(".claude/skills/tracewise/SKILL.md");
    assert!(skill_file.exists());
    let skill_text = fs::read_to_string(&skill_file).unwrap();
    assert!(skill_text.contains("tracewise intent add"));
    assert!(skill_text.contains("tracewise run --why"));
    assert!(skill_text.contains("tracewise pending"));
    assert!(skill_text.contains("tracewise note"));

    let agents_text = fs::read_to_string(&agents_md).unwrap();
    assert!(agents_text.contains("Keep this line."));
    assert!(agents_text.contains("<!-- tracewise:start -->"));
    assert!(agents_text.contains("<!-- tracewise:end -->"));
    assert!(agents_text.contains("tracewise intent add"));

    // 2. Install again (idempotent)
    let before_cfg = fs::read_to_string(&claude_cfg).unwrap();
    let before_agents = fs::read_to_string(&agents_md).unwrap();
    handle_agent_install(&repo_paths, "claude", "project", false).unwrap();
    let after_cfg = fs::read_to_string(&claude_cfg).unwrap();
    let after_agents = fs::read_to_string(&agents_md).unwrap();
    assert_eq!(before_cfg, after_cfg);
    assert_eq!(before_agents, after_agents);

    // 3. Uninstall claude
    handle_agent_uninstall(&repo_paths, "claude", false).unwrap();
    let uninstalled_settings = fs::read_to_string(&claude_cfg).unwrap();
    assert!(uninstalled_settings.contains("echo mine"));
    assert!(!uninstalled_settings.contains("tracewise hook"));
    assert!(!skill_file.exists());

    let uninstalled_agents = fs::read_to_string(&agents_md).unwrap();
    assert!(uninstalled_agents.contains("Keep this line."));
    assert!(!uninstalled_agents.contains("<!-- tracewise:start -->"));
}

#[test]
fn a_config_missing_hook_events_is_reported() {
    let full = "tracewise hook PreToolUse --from cursor tracewise hook PostToolUse --from cursor tracewise hook Stop --from cursor tracewise hook SessionEnd --from cursor";
    assert!(super::missing_events("cursor", full).is_empty());
    let partial = "tracewise hook PreToolUse --from cursor tracewise hook Stop --from cursor";
    assert_eq!(super::missing_events("cursor", partial), ["PostToolUse", "SessionEnd"]);
    // the same text is for another agent: nothing counts
    assert_eq!(super::missing_events("codex", partial).len(), 4);
    // claude has two more events, antigravity one fewer
    let claude = full.replace("cursor", "claude");
    assert_eq!(super::missing_events("claude", &claude), ["UserPromptSubmit", "PostToolUseFailure"]);
    assert!(super::missing_events(
        "antigravity",
        "tracewise hook PreToolUse --from antigravity tracewise hook PostToolUse --from antigravity tracewise hook Stop --from antigravity"
    )
    .is_empty());
}

#[test]
fn claude_install_adds_deny_rules_for_tracewises_own_files_and_keeps_the_persons_own() {
    let tmp = tempdir().unwrap();
    let work_tree = tmp.path().to_path_buf();
    let git_dir = work_tree.join(".git");
    fs::create_dir_all(&git_dir).unwrap();
    let repo_paths = RepoPaths { work_tree: work_tree.clone(), git_dir: git_dir.clone(), git_common_dir: git_dir };
    let cfg = work_tree.join(".claude/settings.json");
    fs::create_dir_all(cfg.parent().unwrap()).unwrap();
    fs::write(&cfg, r#"{"permissions":{"deny":["Edit(secrets/**)"]}}"#).unwrap();

    handle_agent_install(&repo_paths, "claude", "project", false).unwrap();
    handle_agent_install(&repo_paths, "claude", "project", false).unwrap(); // twice: no duplicates
    let v: Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let deny: Vec<&str> = v["permissions"]["deny"].as_array().unwrap().iter().filter_map(|d| d.as_str()).collect();
    assert_eq!(deny.iter().filter(|d| **d == "Edit(**/.git/tracewise/**)").count(), 1);
    for rule in CLAUDE_DENY {
        assert!(deny.contains(&rule), "{rule} missing");
    }
    assert!(deny.contains(&"Edit(secrets/**)"));

    handle_agent_uninstall(&repo_paths, "claude", false).unwrap();
    let v: Value = serde_json::from_str(&fs::read_to_string(&cfg).unwrap()).unwrap();
    let deny: Vec<&str> = v["permissions"]["deny"].as_array().unwrap().iter().filter_map(|d| d.as_str()).collect();
    assert_eq!(deny, vec!["Edit(secrets/**)"]);
}
