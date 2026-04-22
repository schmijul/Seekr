use crate::core::parser::{ChunkRecord, SymbolRecord};
use crate::core::workspace::{detect_repo_meta, now_ts, workspace_id_for_root, WorkspaceRecord};
use rusqlite::{params, Connection};

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA foreign_keys=ON;

        CREATE TABLE IF NOT EXISTS workspaces (
            workspace_id TEXT PRIMARY KEY,
            root_path TEXT NOT NULL UNIQUE,
            repo_type TEXT NOT NULL,
            head_ref TEXT,
            head_commit TEXT,
            index_state TEXT NOT NULL,
            last_snapshot_at INTEGER,
            last_incremental_at INTEGER,
            schema_version INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS files (
            file_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            path TEXT NOT NULL,
            language TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            mtime_ns INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            parse_status TEXT NOT NULL,
            is_generated INTEGER NOT NULL DEFAULT 0,
            content TEXT NOT NULL,
            last_indexed_at INTEGER NOT NULL,
            UNIQUE(workspace_id, path)
        );

        CREATE TABLE IF NOT EXISTS chunks (
            chunk_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            file_id TEXT NOT NULL,
            symbol_id TEXT,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            text_hash TEXT NOT NULL,
            text TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS symbols (
            symbol_id TEXT PRIMARY KEY,
            file_id TEXT NOT NULL,
            workspace_id TEXT NOT NULL,
            language TEXT NOT NULL,
            name TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            signature TEXT,
            container_symbol_id TEXT,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            visibility TEXT,
            lsp_backed INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS "references" (
            reference_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            symbol_id TEXT NOT NULL,
            file_id TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            start_col INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            end_col INTEGER NOT NULL,
            source TEXT NOT NULL,
            confidence REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS snapshots (
            snapshot_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            head_commit TEXT,
            started_at INTEGER NOT NULL,
            completed_at INTEGER,
            status TEXT NOT NULL,
            files_seen INTEGER NOT NULL DEFAULT 0,
            files_indexed INTEGER NOT NULL DEFAULT 0,
            files_failed INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS index_jobs (
            job_id TEXT PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            job_type TEXT NOT NULL,
            status TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            started_at INTEGER,
            completed_at INTEGER,
            payload_json TEXT
        );

        CREATE TABLE IF NOT EXISTS index_errors (
            id INTEGER PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            path TEXT,
            message TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS watch_events (
            id INTEGER PRIMARY KEY,
            workspace_id TEXT NOT NULL,
            path TEXT NOT NULL,
            event_kind TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS language_capabilities (
            workspace_id TEXT NOT NULL,
            language TEXT NOT NULL,
            available INTEGER NOT NULL,
            configured INTEGER NOT NULL,
            active INTEGER NOT NULL,
            failed INTEGER NOT NULL,
            PRIMARY KEY (workspace_id, language)
        );

        CREATE INDEX IF NOT EXISTS idx_files_workspace_path ON files(workspace_id, path);
        CREATE INDEX IF NOT EXISTS idx_chunks_workspace_file ON chunks(workspace_id, file_id);
        CREATE INDEX IF NOT EXISTS idx_symbols_workspace_name ON symbols(workspace_id, name);
        CREATE INDEX IF NOT EXISTS idx_refs_workspace_symbol ON "references"(workspace_id, symbol_id);
        CREATE INDEX IF NOT EXISTS idx_errors_workspace_created ON index_errors(workspace_id, created_at DESC);
        "#,
    )
    .map_err(|e| format!("ensure schema failed: {e}"))
}

pub fn upsert_workspace(conn: &Connection, root_path: &str) -> Result<WorkspaceRecord, String> {
    let workspace_id = workspace_id_for_root(root_path);
    let (repo_type, head_ref, head_commit) = detect_repo_meta(root_path);
    conn.execute(
        r#"
        INSERT INTO workspaces(
            workspace_id, root_path, repo_type, head_ref, head_commit, index_state, schema_version
        ) VALUES (?1, ?2, ?3, ?4, ?5, 'idle', 1)
        ON CONFLICT(root_path) DO UPDATE SET
            repo_type=excluded.repo_type,
            head_ref=excluded.head_ref,
            head_commit=excluded.head_commit
        "#,
        params![workspace_id, root_path, repo_type, head_ref, head_commit],
    )
    .map_err(|e| format!("upsert workspace failed: {e}"))?;
    get_workspace_by_root(conn, root_path)?.ok_or_else(|| "workspace missing after upsert".to_string())
}

pub fn get_workspace_by_root(conn: &Connection, root_path: &str) -> Result<Option<WorkspaceRecord>, String> {
    let mut stmt = conn
        .prepare(
            r#"
            SELECT workspace_id, root_path, repo_type, head_ref, head_commit, index_state,
                   last_snapshot_at, last_incremental_at, schema_version
            FROM workspaces
            WHERE root_path = ?1
            "#,
        )
        .map_err(|e| format!("prepare get workspace failed: {e}"))?;
    let mut rows = stmt.query(params![root_path]).map_err(|e| format!("query get workspace failed: {e}"))?;
    let Some(row) = rows.next().map_err(|e| format!("read workspace row failed: {e}"))? else {
        return Ok(None);
    };
    Ok(Some(WorkspaceRecord {
        workspace_id: row.get(0).map_err(|e| format!("read workspace_id failed: {e}"))?,
        root_path: row.get(1).map_err(|e| format!("read root_path failed: {e}"))?,
        repo_type: row.get(2).map_err(|e| format!("read repo_type failed: {e}"))?,
        head_ref: row.get(3).map_err(|e| format!("read head_ref failed: {e}"))?,
        head_commit: row.get(4).map_err(|e| format!("read head_commit failed: {e}"))?,
        index_state: row.get(5).map_err(|e| format!("read index_state failed: {e}"))?,
        last_snapshot_at: row.get(6).map_err(|e| format!("read last_snapshot_at failed: {e}"))?,
        last_incremental_at: row.get(7).map_err(|e| format!("read last_incremental_at failed: {e}"))?,
        schema_version: row.get(8).map_err(|e| format!("read schema_version failed: {e}"))?,
    }))
}

pub fn set_workspace_state(conn: &Connection, workspace_id: &str, state: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE workspaces SET index_state = ?1 WHERE workspace_id = ?2",
        params![state, workspace_id],
    )
    .map_err(|e| format!("set workspace state failed: {e}"))?;
    Ok(())
}

pub fn begin_snapshot(conn: &Connection, workspace_id: &str, head_commit: Option<&str>) -> Result<String, String> {
    let snapshot_id = format!("snap_{}_{}", workspace_id, now_ts());
    conn.execute(
        r#"
        INSERT INTO snapshots(snapshot_id, workspace_id, head_commit, started_at, status)
        VALUES (?1, ?2, ?3, ?4, 'running')
        "#,
        params![snapshot_id, workspace_id, head_commit, now_ts()],
    )
    .map_err(|e| format!("begin snapshot failed: {e}"))?;
    set_workspace_state(conn, workspace_id, "snapshot_running")?;
    Ok(snapshot_id)
}

pub fn finish_snapshot(
    conn: &Connection,
    snapshot_id: &str,
    workspace_id: &str,
    files_seen: i64,
    files_indexed: i64,
    files_failed: i64,
) -> Result<(), String> {
    let now = now_ts();
    conn.execute(
        r#"
        UPDATE snapshots
        SET completed_at = ?1, status = 'ok', files_seen = ?2, files_indexed = ?3, files_failed = ?4
        WHERE snapshot_id = ?5
        "#,
        params![now, files_seen, files_indexed, files_failed, snapshot_id],
    )
    .map_err(|e| format!("finish snapshot failed: {e}"))?;
    conn.execute(
        r#"
        UPDATE workspaces
        SET index_state = 'idle', last_snapshot_at = ?1
        WHERE workspace_id = ?2
        "#,
        params![now, workspace_id],
    )
    .map_err(|e| format!("update workspace after snapshot failed: {e}"))?;
    Ok(())
}

pub fn fail_snapshot(conn: &Connection, snapshot_id: &str, workspace_id: &str, message: &str) -> Result<(), String> {
    let now = now_ts();
    conn.execute(
        "UPDATE snapshots SET completed_at = ?1, status = 'failed' WHERE snapshot_id = ?2",
        params![now, snapshot_id],
    )
    .map_err(|e| format!("fail snapshot failed: {e}"))?;
    conn.execute(
        "UPDATE workspaces SET index_state = 'error' WHERE workspace_id = ?1",
        params![workspace_id],
    )
    .map_err(|e| format!("update workspace failure state failed: {e}"))?;
    log_index_error(conn, workspace_id, None, message)?;
    Ok(())
}

pub fn log_index_error(conn: &Connection, workspace_id: &str, path: Option<&str>, message: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO index_errors(workspace_id, path, message, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![workspace_id, path, message, now_ts()],
    )
    .map_err(|e| format!("log index error failed: {e}"))?;
    Ok(())
}

pub fn upsert_file(
    conn: &Connection,
    file_id: &str,
    workspace_id: &str,
    path: &str,
    language: &str,
    size_bytes: i64,
    mtime_ns: i64,
    content_hash: &str,
    parse_status: &str,
    content: &str,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO files(file_id, workspace_id, path, language, size_bytes, mtime_ns, content_hash, parse_status, content, last_indexed_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(file_id) DO UPDATE SET
            workspace_id=excluded.workspace_id,
            path=excluded.path,
            language=excluded.language,
            size_bytes=excluded.size_bytes,
            mtime_ns=excluded.mtime_ns,
            content_hash=excluded.content_hash,
            parse_status=excluded.parse_status,
            content=excluded.content,
            last_indexed_at=excluded.last_indexed_at
        "#,
        params![
            file_id,
            workspace_id,
            path,
            language,
            size_bytes,
            mtime_ns,
            content_hash,
            parse_status,
            content,
            now_ts()
        ],
    )
    .map_err(|e| format!("upsert file failed: {e}"))?;
    Ok(())
}

pub fn replace_chunks_for_file(conn: &Connection, workspace_id: &str, file_id: &str, chunks: &[ChunkRecord]) -> Result<(), String> {
    conn.execute(
        "DELETE FROM chunks WHERE workspace_id = ?1 AND file_id = ?2",
        params![workspace_id, file_id],
    )
    .map_err(|e| format!("delete old chunks failed: {e}"))?;
    for c in chunks {
        conn.execute(
            r#"
            INSERT INTO chunks(chunk_id, workspace_id, file_id, symbol_id, kind, start_line, end_line, text_hash, text)
            VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                c.chunk_id,
                workspace_id,
                file_id,
                c.kind,
                c.start_line,
                c.end_line,
                c.text_hash,
                c.text
            ],
        )
        .map_err(|e| format!("insert chunk failed: {e}"))?;
    }
    Ok(())
}

pub fn replace_symbols_for_file(
    conn: &Connection,
    workspace_id: &str,
    file_id: &str,
    language: &str,
    symbols: &[SymbolRecord],
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM symbols WHERE workspace_id = ?1 AND file_id = ?2",
        params![workspace_id, file_id],
    )
    .map_err(|e| format!("delete old symbols failed: {e}"))?;
    for s in symbols {
        conn.execute(
            r#"
            INSERT INTO symbols(
              symbol_id, file_id, workspace_id, language, name, qualified_name, kind, signature,
              container_symbol_id, start_line, end_line, visibility, lsp_backed
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                s.symbol_id,
                file_id,
                workspace_id,
                language,
                s.name,
                s.qualified_name,
                s.kind,
                s.signature,
                s.container_symbol_id,
                s.start_line,
                s.end_line,
                s.visibility,
                if s.lsp_backed { 1 } else { 0 }
            ],
        )
        .map_err(|e| format!("insert symbol failed: {e}"))?;
    }
    Ok(())
}

pub fn prune_missing_files(conn: &Connection, workspace_id: &str, keep_paths: &[String]) -> Result<usize, String> {
    let mut stmt = conn
        .prepare("SELECT file_id, path FROM files WHERE workspace_id = ?1")
        .map_err(|e| format!("prepare file prune scan failed: {e}"))?;
    let rows = stmt
        .query_map(params![workspace_id], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(|e| format!("scan files for prune failed: {e}"))?;
    let mut removed = 0usize;
    for row in rows {
        let (file_id, path) = row.map_err(|e| format!("read prune row failed: {e}"))?;
        if keep_paths.iter().any(|k| k == &path) {
            continue;
        }
        conn.execute("DELETE FROM chunks WHERE workspace_id = ?1 AND file_id = ?2", params![workspace_id, file_id.clone()])
            .map_err(|e| format!("delete chunks during prune failed: {e}"))?;
        conn.execute("DELETE FROM symbols WHERE workspace_id = ?1 AND file_id = ?2", params![workspace_id, file_id.clone()])
            .map_err(|e| format!("delete symbols during prune failed: {e}"))?;
        conn.execute("DELETE FROM files WHERE workspace_id = ?1 AND file_id = ?2", params![workspace_id, file_id])
            .map_err(|e| format!("delete file during prune failed: {e}"))?;
        removed += 1;
    }
    Ok(removed)
}
