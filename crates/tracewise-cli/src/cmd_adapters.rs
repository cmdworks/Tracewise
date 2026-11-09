//! `tracewise agent list`, `tracewise agent check` and install/uninstall for agents described by adapter files.
use crate::hook::adapter::{self, Adapter, BUILTIN};
use crate::RepoPaths;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

type Res = Result<(), Box<dyn std::error::Error>>;

pub fn handle_agent_list(repo_paths: &RepoPaths, json: bool) -> Res {
    let mut rows: Vec<(String, &str, Option<String>)> = BUILTIN.iter().map(|n| (n.to_string(), "builtin", None)).collect();
    rows.extend(adapter::list_files(Some(&repo_paths.work_tree)));
    if json {
        let v: Vec<Value> = rows.iter().map(|(n, s, e)| json!({"name": n, "source": s, "error": e})).collect();
        println!("{}", serde_json::to_string(&v)?);
    } else {
        for (n, s, e) in rows {
            println!("{n:<14} {s:<8} {}", e.map(|e| format!("BROKEN: {e}")).unwrap_or_default());
        }
    }
    Ok(())
}

/// `tracewise agent check <file> <payload.json>`: what the payload would become, without running the agent.
pub fn handle_agent_check(file: &Path, payload: &Path, json: bool) -> Res {
    let fail = |msg: String| -> Res {
        eprintln!("tracewise: {msg}");
        std::process::exit(1)
    };
    let body = match fs::read_to_string(file) {
        Ok(b) => b,
        Err(e) => return fail(format!("cannot read adapter file {}: {e}", file.display())),
    };
    let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("adapter");
    let adapter = match Adapter::parse(stem, &body) {
        Ok(a) => a,
        Err(e) => return fail(format!("{}: {e}", file.display())),
    };
    let text = match fs::read_to_string(payload) {
        Ok(t) => t,
        Err(e) => return fail(format!("cannot read payload file {}: {e}", payload.display())),
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => return fail(format!("{} is not JSON: {e}", payload.display())),
    };
    let t = adapter.translate("", &value);
    if json {
        println!("{}", serde_json::to_string(&t)?);
    } else {
        println!(
            "event: {}\nsession: {}\npaths: {}\ncommand: {}",
            t.event.as_deref().unwrap_or("(none: allowed)"),
            t.session,
            t.paths.join(", "),
            t.command.as_deref().unwrap_or("")
        );
    }
    Ok(())
}

fn merge(dst: &mut Value, src: &Value) {
    match (dst, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, v) in s {
                merge(d.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (Value::Array(d), Value::Array(s)) => {
            for v in s {
                if !d.contains(v) {
                    d.push(v.clone());
                }
            }
        }
        (d, s) => *d = s.clone(),
    }
}

/// Removes what `merge` added; `true` when `dst` is now empty and can go.
fn unmerge(dst: &mut Value, src: &Value) -> bool {
    match (&mut *dst, src) {
        (Value::Object(d), Value::Object(s)) => {
            for (k, v) in s {
                if d.get_mut(k).is_some_and(|x| unmerge(x, v)) {
                    d.remove(k);
                }
            }
            d.is_empty()
        }
        (Value::Array(d), Value::Array(s)) => {
            d.retain(|x| !s.contains(x));
            d.is_empty()
        }
        (d, s) => d == s,
    }
}

/// Installs an agent that is described by an adapter file; `Ok(false)` when `name` is not one.
pub fn install(repo_paths: &RepoPaths, name: &str, json: bool) -> Result<bool, Box<dyn std::error::Error>> {
    let Some((adapter, _)) = adapter::find(name, Some(&repo_paths.work_tree)).map_err(|e| format!("tracewise: {e}"))? else {
        return Ok(false);
    };
    let Some((file, content)) = adapter.install else {
        eprintln!("tracewise: adapter {name} has no [install] section; nothing to write");
        std::process::exit(1);
    };
    let target = repo_paths.work_tree.join(&file);
    let mut current: Value = fs::read_to_string(&target).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_else(|| json!({}));
    merge(&mut current, &serde_json::from_str::<Value>(&content)?);
    if let Some(dir) = target.parent() {
        fs::create_dir_all(dir)?;
    }
    let text = serde_json::to_string_pretty(&current)?;
    if fs::read_to_string(&target).ok().as_deref() != Some(text.as_str()) {
        fs::write(&target, text)?;
    }
    let mut files = vec![file];
    files.push(crate::cmd_agents::install_agents_md_block(repo_paths)?);
    if json {
        println!("{}", json!({"agent": name, "files": files}));
    } else {
        println!("Installed tracewise adapter for {name}:");
        files.iter().for_each(|f| println!("  - {f}"));
    }
    Ok(true)
}

pub fn uninstall(repo_paths: &RepoPaths, name: &str, json: bool) -> Result<bool, Box<dyn std::error::Error>> {
    let Some((adapter, _)) = adapter::find(name, Some(&repo_paths.work_tree)).map_err(|e| format!("tracewise: {e}"))? else {
        return Ok(false);
    };
    if let Some((file, content)) = adapter.install {
        let target = repo_paths.work_tree.join(file);
        if let Some(mut current) = fs::read_to_string(&target).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
            if unmerge(&mut current, &serde_json::from_str::<Value>(&content)?) {
                let _ = fs::remove_file(&target);
            } else {
                fs::write(&target, serde_json::to_string_pretty(&current)?)?;
            }
        }
    }
    crate::cmd_agents::remove_agents_md_block(repo_paths)?;
    if json {
        println!("{}", json!({"agent": name, "uninstalled": true}));
    } else {
        println!("Uninstalled tracewise adapter for {name}");
    }
    Ok(true)
}

#[cfg(test)]
#[path = "test_cmd_adapters.rs"]
mod tests;
