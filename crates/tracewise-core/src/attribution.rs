use crate::db::Database;
use crate::model::{glob_match, Actor, Window};
use rusqlite::Result;

/// Single shared window-attribution routine (R07, Criterion 4 in plan-v0.2).
/// Attributes changes to a window, upgrades actor confidence, and links covering intents.
/// Used by shell precmd, agent post-command, and trail run.
pub fn attribute_window(db: &Database, window: &Window, actor: &Actor, predicted: bool, reads_only: bool) -> Result<Vec<String>> {
    if reads_only {
        return Ok(Vec::new());
    }

    let mut attributed = Vec::new();

    // 1. Settle ambiguous changes since window start for this actor
    let settled = db.settle_ambiguous_for(actor, &window.start)?;
    attributed.extend(settled);

    // 2. Claim / upgrade unknown changes recorded during the window (e.g. by filesystem watcher)
    {
        let unknowns = db.find_unknown_changes_since(&window.start)?;
        for u in unknowns {
            let in_window_paths = window.paths.iter().any(|p| p == &u.path || glob_match(p, &u.path));

            let (should_claim, conf) = if predicted {
                if in_window_paths {
                    (true, "exact")
                } else if actor.kind == "agent" {
                    (true, "likely")
                } else {
                    (false, "likely")
                }
            } else {
                (in_window_paths || actor.kind == "human", "likely")
            };

            if should_claim {
                let source = if actor.kind == "human" { "shell" } else { "hook" };
                db.upgrade_change_actor(&u.id, actor, conf, source)?;
                if !attributed.contains(&u.id) {
                    attributed.push(u.id);
                }
            }
        }
    }

    // 3. Include any changes already recorded for this actor during the window
    for ch in db.list_changes_since(&window.start)? {
        if ch.actor.is_same(actor) && !attributed.contains(&ch.id) {
            attributed.push(ch.id);
        }
    }

    // 4. Link changes without an intent to covering intent
    if let Some(ref name) = actor.name {
        for cid in &attributed {
            if let Some(ch) = db.get_change(cid)? {
                if ch.intent.is_none() {
                    if let Some(intent) = db.find_covering_intent(name, actor.session.as_deref(), &ch.path)? {
                        db.link_intent_to_change(cid, &intent.id)?;
                    }
                }
            }
        }
    }

    Ok(attributed)
}

#[cfg(test)]
#[path = "test_attribution.rs"]
mod tests;
