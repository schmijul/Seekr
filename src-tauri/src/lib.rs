mod config;
mod crawler;
mod db;
mod models;
mod pdf;
mod search;
mod state;
mod watcher;

use models::{HealthStatus, ReindexStatus, SearchResult};
use state::AppState;
use std::fs;
use std::path::PathBuf;
use tauri::{Manager, State};

fn resolve_db_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("resolve app data dir failed: {e}"))?;

    fs::create_dir_all(&data_dir).map_err(|e| format!("create app data dir failed: {e}"))?;

    Ok(data_dir.join("seekr.db"))
}

fn restart_watchers(app_state: &State<'_, AppState>, db_path: PathBuf, roots: &[String]) -> Result<(), String> {
    let watchers = watcher::build_watchers(db_path, roots)?;
    let mut shared = app_state
        .watchers
        .lock()
        .map_err(|_| String::from("watcher lock poisoned"))?;
    *shared = watchers;
    Ok(())
}

#[tauri::command]
fn init_backend(app: tauri::AppHandle, app_state: State<'_, AppState>) -> Result<HealthStatus, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    db::ensure_schema(&conn)?;

    let mut shared = app_state
        .db_path
        .lock()
        .map_err(|_| String::from("db path lock poisoned"))?;
    *shared = Some(db_path.clone());

    Ok(HealthStatus {
        ok: true,
        db_path: db_path.display().to_string(),
    })
}

#[tauri::command]
fn set_index_roots(app: tauri::AppHandle, roots: Vec<String>) -> Result<Vec<String>, String> {
    let db_path = resolve_db_path(&app)?;
    let mut conn = db::open_or_create(&db_path)?;
    db::ensure_schema(&conn)?;
    config::ensure_config_tables(&conn)?;
    config::set_roots(&mut conn, &roots)?;
    let saved = config::get_roots(&conn)?;
    let state = app.state::<AppState>();
    restart_watchers(&state, db_path, &saved)?;
    Ok(saved)
}

#[tauri::command]
fn get_index_roots(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    db::ensure_schema(&conn)?;
    config::ensure_config_tables(&conn)?;
    config::get_roots(&conn)
}

#[tauri::command]
fn run_full_reindex(app: tauri::AppHandle) -> Result<ReindexStatus, String> {
    let db_path = resolve_db_path(&app)?;
    let mut conn = db::open_or_create(&db_path)?;
    db::ensure_schema(&conn)?;
    config::ensure_config_tables(&conn)?;
    let roots = config::get_roots(&conn)?;
    let stats = crawler::crawl_and_index(&mut conn, &roots)?;

    Ok(ReindexStatus {
        indexed: stats.indexed,
        removed: stats.removed,
        failed: stats.failed,
    })
}

#[tauri::command]
fn search_index(app: tauri::AppHandle, query: String, limit: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    db::ensure_schema(&conn)?;

    let found = search::search(&conn, &query, limit.unwrap_or(100))?;
    let mut out = Vec::with_capacity(found.len());
    for row in found {
        out.push(SearchResult {
            title: row.title,
            path: row.path,
            snippet: row.snippet,
            file_type: row.file_type,
            modified_ts: row.modified_ts,
        });
    }

    Ok(out)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            let db_path = resolve_db_path(app.handle())?;
            let conn = db::open_or_create(&db_path)?;
            db::ensure_schema(&conn)?;
            config::ensure_config_tables(&conn)?;
            let roots = config::get_roots(&conn)?;
            let state = app.state::<AppState>();
            restart_watchers(&state, db_path, &roots)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            init_backend,
            set_index_roots,
            get_index_roots,
            run_full_reindex,
            search_index
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
