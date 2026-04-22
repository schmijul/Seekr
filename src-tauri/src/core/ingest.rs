use crate::core::index_store;
use crate::core::parser;
use crate::core::scanner;
use crate::core::text_index::{self, IndexedDoc};
use crate::core::workspace;
use rusqlite::Connection;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotReport {
    pub workspace_id: String,
    pub snapshot_id: String,
    pub files_seen: usize,
    pub files_indexed: usize,
    pub files_failed: usize,
    pub removed: usize,
}

fn file_id(workspace_id: &str, rel_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(workspace_id.as_bytes());
    hasher.update(rel_path.as_bytes());
    format!("file_{}", hex::encode(hasher.finalize()))
}

pub fn build_snapshot(conn: &Connection, index_dir: &Path, root_path: &str) -> Result<SnapshotReport, String> {
    index_store::ensure_schema(conn)?;
    let ws = index_store::upsert_workspace(conn, root_path)?;
    let snapshot_id = index_store::begin_snapshot(conn, &ws.workspace_id, ws.head_commit.as_deref())?;

    let candidates = scanner::scan_workspace(root_path)?;
    let mut tantivy_docs = Vec::new();
    let mut keep_paths = Vec::new();
    let mut indexed = 0usize;
    let mut failed = 0usize;
    for cand in &candidates {
        let file_id = file_id(&ws.workspace_id, &cand.rel_path);
        keep_paths.push(cand.rel_path.clone());
        if let Err(err) = index_store::upsert_file(
            conn,
            &file_id,
            &ws.workspace_id,
            &cand.rel_path,
            &cand.language,
            cand.size_bytes,
            cand.mtime_ns,
            &cand.content_hash,
            "ok",
            &cand.content,
        ) {
            failed += 1;
            let _ = index_store::log_index_error(conn, &ws.workspace_id, Some(&cand.rel_path), &err);
            continue;
        }
        let chunks = parser::chunk_by_lines(&cand.rel_path, &cand.content, 120, 20);
        let symbols = parser::extract_symbols(&cand.rel_path, &cand.language, &cand.content);
        if let Err(err) = index_store::replace_chunks_for_file(conn, &ws.workspace_id, &file_id, &chunks) {
            failed += 1;
            let _ = index_store::log_index_error(conn, &ws.workspace_id, Some(&cand.rel_path), &err);
            continue;
        }
        if let Err(err) = index_store::replace_symbols_for_file(conn, &ws.workspace_id, &file_id, &cand.language, &symbols) {
            failed += 1;
            let _ = index_store::log_index_error(conn, &ws.workspace_id, Some(&cand.rel_path), &err);
            continue;
        }
        tantivy_docs.push(IndexedDoc {
            workspace_id: ws.workspace_id.clone(),
            kind: "file".to_string(),
            path: cand.rel_path.clone(),
            language: cand.language.clone(),
            entity_id: file_id.clone(),
            symbol_name: String::new(),
            content: cand.content.clone(),
            start_line: 1,
            end_line: cand.content.lines().count() as i64,
        });
        for c in &chunks {
            tantivy_docs.push(IndexedDoc {
                workspace_id: ws.workspace_id.clone(),
                kind: "chunk".to_string(),
                path: cand.rel_path.clone(),
                language: cand.language.clone(),
                entity_id: c.chunk_id.clone(),
                symbol_name: String::new(),
                content: c.text.clone(),
                start_line: c.start_line,
                end_line: c.end_line,
            });
        }
        for s in &symbols {
            tantivy_docs.push(IndexedDoc {
                workspace_id: ws.workspace_id.clone(),
                kind: "symbol".to_string(),
                path: cand.rel_path.clone(),
                language: cand.language.clone(),
                entity_id: s.symbol_id.clone(),
                symbol_name: s.name.clone(),
                content: s.signature.clone().unwrap_or_default(),
                start_line: s.start_line,
                end_line: s.end_line,
            });
        }
        indexed += 1;
    }
    let removed = index_store::prune_missing_files(conn, &ws.workspace_id, &keep_paths)?;
    if let Err(err) = text_index::replace_workspace_docs(index_dir, &ws.workspace_id, &tantivy_docs) {
        index_store::fail_snapshot(conn, &snapshot_id, &ws.workspace_id, &err)?;
        return Err(err);
    }
    index_store::finish_snapshot(
        conn,
        &snapshot_id,
        &ws.workspace_id,
        candidates.len() as i64,
        indexed as i64,
        failed as i64,
    )?;

    Ok(SnapshotReport {
        workspace_id: ws.workspace_id,
        snapshot_id,
        files_seen: candidates.len(),
        files_indexed: indexed,
        files_failed: failed,
        removed,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub workspace_id: String,
    pub root_path: String,
    pub repo_type: String,
    pub head_ref: Option<String>,
    pub head_commit: Option<String>,
    pub index_state: String,
    pub last_snapshot_at: Option<i64>,
    pub last_incremental_at: Option<i64>,
}

pub fn open_workspace(conn: &Connection, root_path: &str) -> Result<WorkspaceStatus, String> {
    index_store::ensure_schema(conn)?;
    let rec = index_store::upsert_workspace(conn, root_path)?;
    Ok(WorkspaceStatus {
        workspace_id: rec.workspace_id,
        root_path: rec.root_path,
        repo_type: rec.repo_type,
        head_ref: rec.head_ref,
        head_commit: rec.head_commit,
        index_state: rec.index_state,
        last_snapshot_at: rec.last_snapshot_at,
        last_incremental_at: rec.last_incremental_at,
    })
}

pub fn get_workspace_status(conn: &Connection, root_path: &str) -> Result<WorkspaceStatus, String> {
    index_store::ensure_schema(conn)?;
    let rec = index_store::get_workspace_by_root(conn, root_path)?
        .ok_or_else(|| format!("workspace not found: {root_path}"))?;
    Ok(WorkspaceStatus {
        workspace_id: rec.workspace_id,
        root_path: rec.root_path,
        repo_type: rec.repo_type,
        head_ref: rec.head_ref,
        head_commit: rec.head_commit,
        index_state: rec.index_state,
        last_snapshot_at: rec.last_snapshot_at,
        last_incremental_at: rec.last_incremental_at,
    })
}

pub fn resolve_workspace_id(conn: &Connection, root_path: &str) -> Result<String, String> {
    if let Some(rec) = index_store::get_workspace_by_root(conn, root_path)? {
        return Ok(rec.workspace_id);
    }
    Ok(workspace::workspace_id_for_root(root_path))
}
