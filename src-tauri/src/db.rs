use rusqlite::Connection;
use rusqlite::params;
use std::collections::HashSet;
use std::path::Path;

pub fn open_or_create(db_path: &Path) -> Result<Connection, String> {
    Connection::open(db_path).map_err(|e| format!("open db failed: {e}"))
}

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA foreign_keys=ON;

        CREATE TABLE IF NOT EXISTS indexed_files (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            extension TEXT NOT NULL,
            modified_ts INTEGER NOT NULL,
            indexed_ts INTEGER NOT NULL,
            content TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS indexed_files_fts USING fts5(
            path,
            title,
            content,
            tokenize='porter unicode61'
        );

        CREATE TRIGGER IF NOT EXISTS indexed_files_ai AFTER INSERT ON indexed_files BEGIN
          INSERT INTO indexed_files_fts(rowid, path, title, content)
          VALUES (new.id, new.path, new.title, new.content);
        END;

        CREATE TRIGGER IF NOT EXISTS indexed_files_ad AFTER DELETE ON indexed_files BEGIN
          INSERT INTO indexed_files_fts(indexed_files_fts, rowid, path, title, content)
          VALUES ('delete', old.id, old.path, old.title, old.content);
        END;

        CREATE TRIGGER IF NOT EXISTS indexed_files_au AFTER UPDATE ON indexed_files BEGIN
          INSERT INTO indexed_files_fts(indexed_files_fts, rowid, path, title, content)
          VALUES ('delete', old.id, old.path, old.title, old.content);
          INSERT INTO indexed_files_fts(rowid, path, title, content)
          VALUES (new.id, new.path, new.title, new.content);
        END;

        CREATE INDEX IF NOT EXISTS idx_indexed_files_path ON indexed_files(path);
        CREATE INDEX IF NOT EXISTS idx_indexed_files_modified_ts ON indexed_files(modified_ts);
        "#,
    )
    .map_err(|e| format!("schema init failed: {e}"))
}

pub fn upsert_file(
    conn: &Connection,
    path: &str,
    title: &str,
    extension: &str,
    modified_ts: i64,
    indexed_ts: i64,
    content: &str,
) -> Result<(), String> {
    conn.execute(
        r#"
        INSERT INTO indexed_files(path, title, extension, modified_ts, indexed_ts, content)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT(path) DO UPDATE SET
          title=excluded.title,
          extension=excluded.extension,
          modified_ts=excluded.modified_ts,
          indexed_ts=excluded.indexed_ts,
          content=excluded.content
        "#,
        params![path, title, extension, modified_ts, indexed_ts, content],
    )
    .map_err(|e| format!("upsert file failed: {e}"))?;

    Ok(())
}

pub fn delete_missing_outside_seen(
    conn: &Connection,
    roots: &[String],
    seen_paths: &HashSet<String>,
) -> Result<usize, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM indexed_files")
        .map_err(|e| format!("prepare indexed_files scan failed: {e}"))?;

    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("query indexed_files scan failed: {e}"))?;

    let mut to_remove = Vec::new();

    for row in rows {
        let path = row.map_err(|e| format!("read indexed file row failed: {e}"))?;
        let under_roots = roots.iter().any(|root| path.starts_with(root));
        if under_roots && !seen_paths.contains(&path) {
            to_remove.push(path);
        }
    }

    let mut removed = 0usize;
    for path in to_remove {
        conn.execute("DELETE FROM indexed_files WHERE path = ?1", params![path])
            .map_err(|e| format!("delete stale file failed: {e}"))?;
        removed += 1;
    }

    Ok(removed)
}

pub fn delete_file_by_path(conn: &Connection, path: &str) -> Result<(), String> {
    conn.execute("DELETE FROM indexed_files WHERE path = ?1", params![path])
        .map_err(|e| format!("delete by path failed: {e}"))?;
    Ok(())
}
