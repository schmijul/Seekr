use rusqlite::{params, Connection};

#[derive(Debug)]
pub struct SearchRow {
    pub title: String,
    pub path: String,
    pub snippet: String,
    pub file_type: String,
    pub modified_ts: i64,
}

pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SearchRow>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
              f.title,
              f.path,
              snippet(indexed_files_fts, 2, '<mark>', '</mark>', ' … ', 16) AS snippet,
              f.extension,
              f.modified_ts
            FROM indexed_files_fts
            JOIN indexed_files f ON f.id = indexed_files_fts.rowid
            WHERE indexed_files_fts MATCH ?1
            ORDER BY bm25(indexed_files_fts)
            LIMIT ?2
            "#,
        )
        .map_err(|e| format!("prepare search failed: {e}"))?;

    let rows = stmt
        .query_map(params![q, limit as i64], |row| {
            Ok(SearchRow {
                title: row.get(0)?,
                path: row.get(1)?,
                snippet: row.get(2)?,
                file_type: row.get(3)?,
                modified_ts: row.get(4)?,
            })
        })
        .map_err(|e| format!("query search failed: {e}"))?;

    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("read search row failed: {e}"))?);
    }

    Ok(out)
}
