use crate::{crawler, db};
use notify::{Event, RecommendedWatcher, RecursiveMode, Result as NotifyResult, Watcher};
use std::path::Path;
use std::path::PathBuf;

const EXCLUDED_DIRS: [&str; 6] = [".git", "node_modules", "dist", "target", "venv", "__pycache__"];

fn has_excluded_segment(path: &Path) -> bool {
    path.components().any(|c| {
        let text = c.as_os_str().to_string_lossy();
        EXCLUDED_DIRS.iter().any(|x| *x == text)
    })
}

fn handle_event(db_path: &Path, event: Event) {
    let mut conn = match db::open_or_create(db_path) {
        Ok(x) => x,
        Err(_) => return,
    };

    for path in event.paths {
        if has_excluded_segment(&path) {
            continue;
        }

        if path.is_dir() {
            continue;
        }

        if !crawler::is_supported_text_file(&path) {
            continue;
        }

        if path.exists() {
            let _ = crawler::index_single_path(&mut conn, &path);
        } else {
            let _ = db::delete_file_by_path(&conn, &path.display().to_string());
        }
    }
}

pub fn build_watchers(db_path: PathBuf, roots: &[String]) -> Result<Vec<RecommendedWatcher>, String> {
    let mut out = Vec::new();

    for root in roots {
        let db_path_for_cb = db_path.clone();
        let mut watcher = notify::recommended_watcher(move |res: NotifyResult<Event>| {
            if let Ok(event) = res {
                handle_event(&db_path_for_cb, event);
            }
        })
        .map_err(|e| format!("create watcher failed: {e}"))?;

        watcher
            .watch(Path::new(root), RecursiveMode::Recursive)
            .map_err(|e| format!("watch root failed ({root}): {e}"))?;

        out.push(watcher);
    }

    Ok(out)
}
