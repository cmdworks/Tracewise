//! Commands: Share, Push, Fetch, Rebuild.
use super::*;

pub(crate) fn run(json: bool, repo_paths: &RepoPaths, command: Commands) {
    let profile = tracewise_core::profile::ProfileConfig::from_root(&repo_paths.work_tree);
    if !profile.share {
        eprintln!("trail: share profile is not enabled; add '[profile] share = true' to .trailconfig to enable");
        process::exit(1);
    }

    match command {
        Commands::Share { action } => handle_share_action(json, repo_paths, action),
        Commands::Push => handle_push(json, repo_paths),
        Commands::Fetch => handle_fetch(json, repo_paths),
        Commands::Rebuild => handle_rebuild(json, repo_paths),
        _ => unreachable!("dispatched to the wrong group"),
    }
}

fn handle_share_action(json: bool, repo_paths: &RepoPaths, action: ShareAction) {
    match action {
        ShareAction::Init { remote, level, hidden, encrypt } => {
            // no --remote: the project's own origin, with hidden refs
            let (given, hidden, store) = match remote {
                Some(r) => (r, hidden, "url"),
                None => match tracewise_core::share::origin_url(&repo_paths.work_tree) {
                    Some(o) => (o, true, "origin"),
                    None => {
                        eprintln!("trail: this project has no `origin` remote; pass --remote <url> for a separate companion repository");
                        process::exit(1);
                    }
                },
            };
            let level = level.unwrap_or_else(|| "summary".to_string());
            let remote = tracewise_core::share::strip_credentials(&given);
            if remote != given {
                eprintln!("trail: the password in the remote URL was not stored; git's credential helper supplies it");
            }
            if level != "full" && level != "summary" && level != "off" {
                eprintln!("trail: invalid sharing level '{}', must be full, summary, or off", level);
                process::exit(1);
            }
            if level == "full" && store == "origin" {
                eprintln!("trail: warning: level full on the project's own remote lets everyone who can read the repository read file contents and reasons; summary shares no contents");
            }
            if let Err(e) = tracewise_core::share::write_trailconfig_full(&repo_paths.work_tree, &remote, &level, hidden, encrypt) {
                eprintln!("trail: cannot write .trailconfig: {e}");
                process::exit(1);
            }
            if let Err(e) = tracewise_core::share::trust_remote(&repo_paths.data_dir(), &remote) {
                eprintln!("trail: cannot record approved remote: {e}");
                process::exit(1);
            }
            let key_id = if encrypt {
                match tracewise_core::share::crypt::ensure_key(&repo_paths.data_dir()) {
                    Ok(id) => Some(id),
                    Err(e) => {
                        eprintln!("trail: cannot make the encryption key: {e}");
                        process::exit(1);
                    }
                }
            } else {
                None
            };
            let refs = if hidden { "hidden" } else { "branches" };
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "remote": remote, "level": level, "refs": refs, "store": store, "encrypt": encrypt, "key_id": key_id })
                );
            } else {
                println!("Sharing initialized: remote = {remote} ({store}), level = {level}, refs = {refs}");
                if let Some(id) = key_id {
                    println!("Encryption is on; the project key {id} is kept in this clone's data folder, never in the repository.");
                    println!("Run `trail share key export` now and keep the result in a password manager: without it a lost clone cannot read the shared history.");
                }
            }
        }
        ShareAction::Status => handle_share_status(json, repo_paths),
        ShareAction::Key { action } => handle_key_action(json, repo_paths, action),
        ShareAction::Keys { forget } => {
            let dir = repo_paths.data_dir();
            if let Some(device) = forget {
                match tracewise_core::share::sign::forget(&dir, &device) {
                    Ok(had) => println!(
                        "{}",
                        if had { format!("Forgot the pinned key of {device}") } else { format!("No key pinned for {device}") }
                    ),
                    Err(e) => {
                        eprintln!("trail: cannot update the pinned keys: {e}");
                        process::exit(1);
                    }
                }
                return;
            }
            let pins = tracewise_core::share::sign::pinned(&dir);
            if json {
                let keys: Vec<_> = pins.iter().map(|(d, k)| serde_json::json!({ "device": d, "key": k })).collect();
                println!("{}", serde_json::json!({ "keys": keys }));
            } else if pins.is_empty() {
                println!("No device keys pinned yet.");
            } else {
                for (d, k) in &pins {
                    println!("{d}  {}", &k[..16]);
                }
            }
        }
        ShareAction::Trust => {
            let Some(cfg) = tracewise_core::share::read_trailconfig(&repo_paths.work_tree) else {
                eprintln!("trail: no valid .trailconfig in this repository");
                process::exit(1);
            };
            if let Err(e) = tracewise_core::share::trust_remote(&repo_paths.data_dir(), &cfg.remote) {
                eprintln!("trail: cannot approve remote: {e}");
                process::exit(1);
            }
            if json {
                println!("{}", serde_json::json!({ "remote": cfg.remote, "trusted": true }));
            } else {
                println!("Approved remote {}", cfg.remote);
            }
        }
    }
}

fn handle_push(json: bool, repo_paths: &RepoPaths) {
    if let Some(original) = copy_of(repo_paths) {
        eprintln!(
            "trail: push refused: this folder is a copy of {}; run `trail adopt-copy` first so it does not push under the original's id",
            original.display()
        );
        process::exit(1);
    }
    match tracewise_core::share::push(&repo_paths.work_tree, &repo_paths.data_dir()) {
        Ok(Some((batch_id, count))) => {
            if json {
                println!("{}", serde_json::json!({ "batch": batch_id, "records": count }));
            } else {
                println!("Pushed batch {} with {} record(s).", batch_id, count);
            }
        }
        Ok(None) => {
            if json {
                println!("{}", serde_json::json!({ "batch": serde_json::Value::Null, "records": 0 }));
            } else {
                println!("Nothing to push.");
            }
        }
        Err(e) => {
            eprintln!("trail: push failed: {e}");
            process::exit(1);
        }
    }
}

fn handle_fetch(json: bool, repo_paths: &RepoPaths) {
    match tracewise_core::share::fetch(&repo_paths.work_tree, &repo_paths.data_dir()) {
        Ok(imported) => {
            if json {
                println!("{}", serde_json::json!({ "imported": imported }));
            } else {
                println!("Imported {} record(s).", imported);
            }
        }
        Err(e) => {
            eprintln!("trail: fetch failed: {e}");
            process::exit(1);
        }
    }
}

fn handle_rebuild(json: bool, repo_paths: &RepoPaths) {
    match tracewise_core::share::rebuild(&repo_paths.work_tree, &repo_paths.data_dir()) {
        Ok(count) => {
            if json {
                println!("{}", serde_json::json!({ "rebuilt": true, "imported": count }));
            } else {
                println!("Rebuilt database ({} record(s)).", count);
            }
        }
        Err(e) => {
            eprintln!("trail: rebuild failed: {e}");
            process::exit(1);
        }
    }
}

fn key_fail(msg: &str) -> ! {
    eprintln!("trail: {msg}");
    process::exit(1);
}

fn handle_key_action(json: bool, repo_paths: &RepoPaths, action: KeyAction) {
    use tracewise_core::share::crypt;
    let dir = repo_paths.data_dir();
    // the key opens the whole shared history: an agent must not read, add or change it
    if !matches!(action, KeyAction::List) && tracewise_core::model::Actor::from_env().is_some_and(|a| a.kind == "agent") {
        key_fail("refused: agents cannot handle the project key");
    }
    match action {
        KeyAction::Export { id } => {
            match crypt::export_key(&dir, id.as_deref()) {
                Ok(text) => {
                    if let Some(used) = id.clone().or(crypt::load(&dir).current) {
                        crypt::note_exported(&dir, &used);
                    }
                    eprintln!("trail: this string is the key itself; anyone who has it can read the shared history. Keep it in a password manager.");
                    println!("{}", if json { serde_json::json!({ "key": text }).to_string() } else { text });
                }
                Err(e) => key_fail(&e.to_string()),
            }
        }
        KeyAction::Import { key, current } => match crypt::import_key(&dir, &key, current) {
            Ok(id) => {
                let now = crypt::load(&dir).current;
                if json {
                    println!("{}", serde_json::json!({ "key_id": id, "current": now.as_deref() == Some(id.as_str()) }));
                } else {
                    println!("Imported key {id}{}.", if now.as_deref() == Some(id.as_str()) { " (current)" } else { "" });
                }
            }
            Err(e) => key_fail(&e.to_string()),
        },
        KeyAction::List => {
            let ring = crypt::load(&dir);
            if json {
                let keys: Vec<_> = ring
                    .keys
                    .iter()
                    .map(|(id, _)| serde_json::json!({ "id": id, "current": ring.current.as_deref() == Some(id.as_str()) }))
                    .collect();
                println!("{}", serde_json::json!({ "keys": keys }));
            } else if ring.keys.is_empty() {
                println!("No key in this clone.");
            } else {
                for (id, _) in &ring.keys {
                    println!("{id}{}", if ring.current.as_deref() == Some(id.as_str()) { "  (current)" } else { "" });
                }
            }
        }
        KeyAction::Rotate => match crypt::add_new_key(&dir) {
            Ok(id) => {
                if json {
                    println!("{}", serde_json::json!({ "key_id": id }));
                } else {
                    println!("New key {id} is current. Give it to the others with `trail share key export`; they run `trail share key import --current`.");
                }
            }
            Err(e) => key_fail(&e.to_string()),
        },
        KeyAction::Rekey { prune_old, yes } => {
            if prune_old && !yes {
                key_fail("--prune-old deletes this device's ref on the remote; add --yes to confirm");
            }
            let Some(cfg) = tracewise_core::share::read_trailconfig(&repo_paths.work_tree) else {
                key_fail("sharing is not set up (.trailconfig)")
            };
            if !cfg.encrypt {
                key_fail("encryption is not on in .trailconfig (`encrypt = true`); nothing to rekey");
            }
            if prune_old {
                if let Err(e) = tracewise_core::share::delete_own_ref(&repo_paths.work_tree, &dir) {
                    key_fail(&format!("could not delete the old ref: {e}"));
                }
            }
            let id = match crypt::add_new_key(&dir) {
                Ok(id) => id,
                Err(e) => key_fail(&e.to_string()),
            };
            let device = tracewise_core::identity::device_for(&dir);
            let marked = Database::open(&dir.join("trail.db")).and_then(|d| d.mark_device_unpushed(&device)).unwrap_or(0);
            match tracewise_core::share::push(&repo_paths.work_tree, &dir) {
                Ok(pushed) => {
                    let (batch, records) = pushed.map(|(b, n)| (Some(b), n)).unwrap_or((None, 0));
                    if json {
                        println!(
                            "{}",
                            serde_json::json!({ "key_id": id, "marked": marked, "batch": batch, "records": records, "pruned": prune_old })
                        );
                    } else {
                        println!("New key {id}; pushed {records} record(s) again. Give the key to the others: `trail share key export`.");
                    }
                }
                Err(e) => key_fail(&format!("the new key {id} is made but the push failed: {e}")),
            }
        }
    }
}

fn handle_share_status(json: bool, repo_paths: &RepoPaths) {
    let dir = repo_paths.data_dir();
    let cfg = tracewise_core::share::read_trailconfig(&repo_paths.work_tree);
    let ring = tracewise_core::share::crypt::load(&dir);
    let warnings = tracewise_core::share::sharing_warnings(&repo_paths.work_tree, &dir);
    let value = match &cfg {
        None => serde_json::json!({ "configured": false, "warnings": warnings }),
        Some(c) => {
            let store =
                if tracewise_core::share::origin_url(&repo_paths.work_tree).is_some_and(|o| o == c.remote) { "origin" } else { "url" };
            serde_json::json!({
                "configured": true,
                "remote": c.remote,
                "store": store,
                "level": c.level,
                "refs": if c.hidden_refs { "hidden" } else { "branches" },
                "encrypt": c.encrypt,
                "key_id": ring.current,
                "key_exported": ring.current.as_deref().is_some_and(|id| tracewise_core::share::crypt::was_exported(&dir, id)),
                "trusted": tracewise_core::share::is_remote_trusted(&dir, &c.remote),
                "warnings": warnings,
            })
        }
    };
    if json {
        println!("{value}");
    } else if cfg.is_none() {
        println!("Sharing is not set up. Run `trail share init`.");
    } else {
        println!(
            "remote {} ({}), level {}, refs {}, encryption {}{}",
            value["remote"].as_str().unwrap_or(""),
            value["store"].as_str().unwrap_or(""),
            value["level"].as_str().unwrap_or(""),
            value["refs"].as_str().unwrap_or(""),
            if value["encrypt"] == true { "on" } else { "off" },
            value["key_id"].as_str().map(|k| format!(" (key {k})")).unwrap_or_default()
        );
        for w in &warnings {
            println!("trail: warning: {w}");
        }
    }
}
