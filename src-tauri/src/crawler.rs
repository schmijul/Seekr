use crate::db;
use rusqlite::Connection;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use walkdir::{DirEntry, WalkDir};

const EXCLUDED_DIRS: [&str; 6] = [".git", "node_modules", "dist", "target", "venv", "__pycache__"];

fn is_excluded(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }

    let name = entry.file_name().to_string_lossy();
    EXCLUDED_DIRS.iter().any(|x| *x == name)
}

fn is_supported_text_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|x| x.to_str()).map(|x| x.to_lowercase()),
        Some(ext) if ext == "txt" || ext == "md"
    )
}

fn unix_ts_now() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(x) => x.as_secs() as i64,
        Err(_) => 0,
    }
}

fn modified_ts(path: &Path) -> i64 {
    match fs::metadata(path).and_then(|m| m.modified()) {
        Ok(m) => match m.duration_since(UNIX_EPOCH) {
            Ok(x) => x.as_secs() as i64,
            Err(_) => 0,
        },
        Err(_) => 0,
    }
}

pub struct IndexStats {
    pub indexed: usize,
    pub removed: usize,
    pub failed: usize,
}

pub fn crawl_and_index(conn: &mut Connection, roots: &[String]) -> Result<IndexStats, String> {
    let mut indexed = 0usize;
    let mut failed = 0usize;
    let mut seen_paths: HashSet<String> = HashSet::new();

    for root in roots {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_excluded(e))
        {
            let entry = match entry {
                Ok(x) => x,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };

            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            if !is_supported_text_file(path) {
                continue;
            }

            let abs = match path.canonicalize() {
                Ok(x) => x,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };
            let path_str = abs.display().to_string();

            let content = match fs::read_to_string(&abs) {
                Ok(x) => x,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };

            let title = abs
                .file_name()
                .and_then(|x| x.to_str())
                .map(String::from)
                .unwrap_or_else(|| String::from("unknown"));

            let ext = abs
                .extension()
                .and_then(|x| x.to_str())
                .map(String::from)
                .unwrap_or_else(|| String::from("txt"));

            let modified = modified_ts(&abs);
            let indexed_ts = unix_ts_now();

            db::upsert_file(
                conn,
                &path_str,
                &title,
                &ext,
                modified,
                indexed_ts,
                &content,
            )?;

            seen_paths.insert(path_str);
            indexed += 1;
        }
    }

    let removed = db::delete_missing_outside_seen(conn, roots, &seen_paths)?;

    Ok(IndexStats {
        indexed,
        removed,
        failed,
    })
}
