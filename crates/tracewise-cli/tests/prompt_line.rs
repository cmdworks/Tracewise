//! B11: with the prompt line on, `trail shell precmd` says on stderr when someone else changed files since the last prompt.
mod common;
use common::Repo;

const SAID: &str = "since your last prompt";

fn precmd(repo: &Repo, line_on: bool) -> String {
    let env: &[(&str, &str)] = if line_on { &[("TRAIL_PROMPT_LINE", "1")] } else { &[] };
    repo.with("human:me", &["shell", "precmd", "--id", "t1", "--exit", "0"], None, env).err
}

#[test]
fn the_line_appears_once_after_someone_else_changed_a_file() {
    let repo = Repo::new("");
    repo.write("a.txt", "base\n");
    assert!(!precmd(&repo, true).contains(SAID), "the first prompt only records the time");
    repo.edit_as("agent:codex:s1", "a.txt", "base\nedited\n");
    let out = precmd(&repo, true);
    assert!(out.contains("1 change in 1 file by codex"), "{out}");
    assert!(out.contains(SAID), "{out}");
    assert!(!precmd(&repo, true).contains(SAID), "said once, not again at the next prompt");
}

#[test]
fn a_change_by_the_person_is_not_announced() {
    let repo = Repo::new("");
    precmd(&repo, true);
    repo.edit_as("human:me", "a.txt", "mine\n");
    assert!(!precmd(&repo, true).contains(SAID));
}

#[test]
fn the_line_is_off_by_default_and_the_config_turns_it_on() {
    let repo = Repo::new("");
    precmd(&repo, false);
    repo.edit_as("agent:codex:s1", "a.txt", "x\n");
    assert!(!precmd(&repo, false).contains(SAID), "off unless asked for");

    let on = Repo::new("[shell]\nprompt_line = \"on\"\n");
    precmd(&on, false);
    on.edit_as("agent:codex:s1", "a.txt", "x\n");
    assert!(precmd(&on, false).contains(SAID), "`[shell] prompt_line = \"on\"` is enough");
}
