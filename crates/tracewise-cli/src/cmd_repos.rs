//! `trail repos`: the repositories one daemon watches (UI project list).
use tracewise_core::db::Database;

/// One row per registered repository that still has a history.
pub(crate) fn rows() -> Vec<serde_json::Value> {
    tracewise_core::repos::list()
        .into_iter()
        .map(|r| {
            let db = r.data_dir.join("trail.db");
            let changes = Database::open(&db).and_then(|d| d.count_changes()).ok();
            let bytes: u64 =
                ["trail.db", "trail.db-wal"].iter().filter_map(|f| std::fs::metadata(r.data_dir.join(f)).ok()).map(|m| m.len()).sum();
            serde_json::json!({ "work_tree": r.work_tree, "data_dir": r.data_dir, "changes": changes, "db_bytes": bytes })
        })
        .collect()
}

pub(crate) fn run(json: bool) {
    let rows = rows();
    if json {
        println!("{}", serde_json::Value::Array(rows));
    } else if rows.is_empty() {
        println!("No repositories are watched yet; run `trail init` in one.");
    } else {
        for r in &rows {
            println!(
                "{}  {} changes  {} KB",
                r["work_tree"].as_str().unwrap_or(""),
                r["changes"].as_u64().unwrap_or(0),
                r["db_bytes"].as_u64().unwrap_or(0) / 1024
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_registered_repositories_with_counts() {
        let home = tempfile::tempdir().unwrap();
        let work = tempfile::tempdir().unwrap();
        let data = work.path().join(".git/trail");
        std::fs::create_dir_all(&data).unwrap();
        Database::open(&data.join("trail.db")).unwrap();
        assert!(tracewise_core::repos::register_in(home.path(), work.path(), &data));
        let listed = tracewise_core::repos::list_in(home.path());
        assert_eq!(listed.len(), 1);
        let n = Database::open(&listed[0].data_dir.join("trail.db")).unwrap().count_changes().unwrap();
        assert_eq!(n, 0);
    }
}
