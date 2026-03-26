use rusqlite::Connection;
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
