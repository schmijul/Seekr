mod config;
pub mod core;
mod crawler;
pub mod db;
mod models;
mod pdf;
mod search;
mod state;
mod watcher;

use core::{ingest, index_store, query};
use models::{
    ChunkHit, DefinitionHit, FileExcerpt, HealthStatus, IndexError, ReindexStatus, ReferenceSearchHit, SearchResult,
    SnapshotStatus, SymbolSearchHit, TextSearchHit, WorkspaceHealth,
};
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

fn resolve_index_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("resolve app data dir failed: {e}"))?;
    fs::create_dir_all(&data_dir).map_err(|e| format!("create app data dir failed: {e}"))?;
    Ok(data_dir.join("seekr-index"))
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
    index_store::ensure_schema(&conn)?;
    config::ensure_config_tables(&conn)?;

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
fn open_workspace(app: tauri::AppHandle, root_path: String) -> Result<WorkspaceHealth, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    ingest::open_workspace(&conn, &root_path)
}

#[tauri::command]
fn get_workspace_status(app: tauri::AppHandle, root_path: String) -> Result<WorkspaceHealth, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    ingest::get_workspace_status(&conn, &root_path)
}

#[tauri::command]
fn start_snapshot_index(app: tauri::AppHandle, root_path: String) -> Result<SnapshotStatus, String> {
    let db_path = resolve_db_path(&app)?;
    let index_dir = resolve_index_dir(&app)?;
    let conn = db::open_or_create(&db_path)?;
    ingest::build_snapshot(&conn, &index_dir, &root_path)
}

#[tauri::command]
fn search_text(
    app: tauri::AppHandle,
    root_path: String,
    query_text: String,
    limit: Option<usize>,
) -> Result<Vec<TextSearchHit>, String> {
    let db_path = resolve_db_path(&app)?;
    let index_dir = resolve_index_dir(&app)?;
    let conn = db::open_or_create(&db_path)?;
    let workspace_id = ingest::resolve_workspace_id(&conn, &root_path)?;
    query::search_text(&index_dir, &workspace_id, &query_text, limit.unwrap_or(50))
}

#[tauri::command]
fn search_symbols(
    app: tauri::AppHandle,
    root_path: String,
    query_text: String,
    limit: Option<usize>,
) -> Result<Vec<SymbolSearchHit>, String> {
    let db_path = resolve_db_path(&app)?;
    let index_dir = resolve_index_dir(&app)?;
    let conn = db::open_or_create(&db_path)?;
    let workspace_id = ingest::resolve_workspace_id(&conn, &root_path)?;
    query::search_symbols(&index_dir, &workspace_id, &query_text, limit.unwrap_or(50))
}

#[tauri::command]
fn lookup_definition(app: tauri::AppHandle, root_path: String, symbol_query: String) -> Result<Option<DefinitionHit>, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    let workspace_id = ingest::resolve_workspace_id(&conn, &root_path)?;
    query::lookup_definition(&conn, &workspace_id, &symbol_query)
}

#[tauri::command]
fn find_references(
    app: tauri::AppHandle,
    root_path: String,
    symbol_id: String,
    limit: Option<usize>,
) -> Result<Vec<ReferenceSearchHit>, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    let workspace_id = ingest::resolve_workspace_id(&conn, &root_path)?;
    query::find_references(&conn, &workspace_id, &symbol_id, limit.unwrap_or(100))
}

#[tauri::command]
fn get_chunk(app: tauri::AppHandle, root_path: String, chunk_id: String) -> Result<Option<ChunkHit>, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    let workspace_id = ingest::resolve_workspace_id(&conn, &root_path)?;
    query::get_chunk(&conn, &workspace_id, &chunk_id)
}

#[tauri::command]
fn get_file_excerpt(
    app: tauri::AppHandle,
    root_path: String,
    file_id: String,
    start_line: i64,
    end_line: i64,
) -> Result<Option<FileExcerpt>, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    let workspace_id = ingest::resolve_workspace_id(&conn, &root_path)?;
    query::get_file_excerpt(&conn, &workspace_id, &file_id, start_line, end_line)
}

#[tauri::command]
fn list_recent_index_errors(
    app: tauri::AppHandle,
    root_path: String,
    limit: Option<usize>,
) -> Result<Vec<IndexError>, String> {
    let db_path = resolve_db_path(&app)?;
    let conn = db::open_or_create(&db_path)?;
    let workspace_id = ingest::resolve_workspace_id(&conn, &root_path)?;
    query::list_recent_index_errors(&conn, &workspace_id, limit.unwrap_or(50))
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
    let index_dir = resolve_index_dir(&app)?;
    let conn = db::open_or_create(&db_path)?;
    db::ensure_schema(&conn)?;
    config::ensure_config_tables(&conn)?;
    let roots = config::get_roots(&conn)?;
    if roots.is_empty() {
        return Ok(ReindexStatus {
            indexed: 0,
            removed: 0,
            failed: 0,
        });
    }

    let mut indexed = 0usize;
    let mut removed = 0usize;
    let mut failed = 0usize;
    for root in roots {
        match ingest::build_snapshot(&conn, &index_dir, &root) {
            Ok(report) => {
                indexed += report.files_indexed;
                removed += report.removed;
                failed += report.files_failed;
            }
            Err(_) => {
                failed += 1;
            }
        }
    }
    Ok(ReindexStatus { indexed, removed, failed })
}

#[tauri::command]
fn search_index(app: tauri::AppHandle, query: String, limit: Option<usize>) -> Result<Vec<SearchResult>, String> {
    let db_path = resolve_db_path(&app)?;
    let index_dir = resolve_index_dir(&app)?;
    let conn = db::open_or_create(&db_path)?;
    config::ensure_config_tables(&conn)?;
    let roots = config::get_roots(&conn)?;
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let ws_id = ingest::resolve_workspace_id(&conn, &roots[0])?;
    let found = query::search_text(&index_dir, &ws_id, &query, limit.unwrap_or(100))?;
    let mut out = Vec::with_capacity(found.len());
    for row in found {
        out.push(SearchResult {
            title: if row.symbol_name.is_empty() {
                row.path.clone()
            } else {
                row.symbol_name.clone()
            },
            path: row.path,
            snippet: row.content_excerpt,
            file_type: row.language,
            modified_ts: 0,
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
            index_store::ensure_schema(&conn)?;
            config::ensure_config_tables(&conn)?;
            let roots = config::get_roots(&conn)?;
            let state = app.state::<AppState>();
            restart_watchers(&state, db_path, &roots)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            init_backend,
            open_workspace,
            get_workspace_status,
            start_snapshot_index,
            search_text,
            search_symbols,
            lookup_definition,
            find_references,
            get_chunk,
            get_file_excerpt,
            list_recent_index_errors,
            set_index_roots,
            get_index_roots,
            run_full_reindex,
            search_index
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
