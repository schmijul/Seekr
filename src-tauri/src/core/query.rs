use crate::core::index_store;
use crate::core::text_index;
use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolLookupHit {
    pub symbol_id: String,
    pub file_id: String,
    pub path: String,
    pub language: String,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub signature: Option<String>,
    pub start_line: i64,
    pub end_line: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceHit {
    pub reference_id: String,
    pub file_id: String,
    pub path: String,
    pub start_line: i64,
    pub start_col: i64,
    pub end_line: i64,
    pub end_col: i64,
    pub source: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkLookupHit {
    pub chunk_id: String,
    pub file_id: String,
    pub path: String,
    pub language: String,
    pub kind: String,
    pub start_line: i64,
    pub end_line: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileExcerptHit {
    pub file_id: String,
    pub path: String,
    pub language: String,
    pub start_line: i64,
    pub end_line: i64,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexErrorHit {
    pub path: Option<String>,
    pub message: String,
    pub created_at: i64,
}

pub fn search_text(index_dir: &Path, workspace_id: &str, query: &str, limit: usize) -> Result<Vec<text_index::SearchHit>, String> {
    text_index::search_text(index_dir, workspace_id, query, limit)
}

pub fn search_symbols(index_dir: &Path, workspace_id: &str, query: &str, limit: usize) -> Result<Vec<text_index::SearchHit>, String> {
    text_index::search_symbols(index_dir, workspace_id, query, limit)
}

pub fn lookup_definition(conn: &Connection, workspace_id: &str, symbol_query: &str) -> Result<Option<SymbolLookupHit>, String> {
    index_store::ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT s.symbol_id, s.file_id, f.path, s.language, s.name, s.qualified_name, s.kind, s.signature, s.start_line, s.end_line
            FROM symbols s
            JOIN files f ON f.file_id = s.file_id
            WHERE s.workspace_id = ?1 AND (s.name = ?2 OR s.qualified_name = ?2)
            ORDER BY s.start_line ASC
            LIMIT 1
            "#,
        )
        .map_err(|e| format!("prepare lookup_definition failed: {e}"))?;
    let mut rows = stmt
        .query(params![workspace_id, symbol_query])
        .map_err(|e| format!("query lookup_definition failed: {e}"))?;
    let Some(row) = rows.next().map_err(|e| format!("read lookup_definition failed: {e}"))? else {
        return Ok(None);
    };
    Ok(Some(SymbolLookupHit {
        symbol_id: row.get(0).map_err(|e| format!("read symbol_id failed: {e}"))?,
        file_id: row.get(1).map_err(|e| format!("read file_id failed: {e}"))?,
        path: row.get(2).map_err(|e| format!("read path failed: {e}"))?,
        language: row.get(3).map_err(|e| format!("read language failed: {e}"))?,
        name: row.get(4).map_err(|e| format!("read name failed: {e}"))?,
        qualified_name: row.get(5).map_err(|e| format!("read qualified_name failed: {e}"))?,
        kind: row.get(6).map_err(|e| format!("read kind failed: {e}"))?,
        signature: row.get(7).map_err(|e| format!("read signature failed: {e}"))?,
        start_line: row.get(8).map_err(|e| format!("read start_line failed: {e}"))?,
        end_line: row.get(9).map_err(|e| format!("read end_line failed: {e}"))?,
    }))
}

pub fn find_references(conn: &Connection, workspace_id: &str, symbol_id: &str, limit: usize) -> Result<Vec<ReferenceHit>, String> {
    index_store::ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT r.reference_id, r.file_id, f.path, r.start_line, r.start_col, r.end_line, r.end_col, r.source, r.confidence
            FROM "references" r
            JOIN files f ON f.file_id = r.file_id
            WHERE r.workspace_id = ?1 AND r.symbol_id = ?2
            ORDER BY r.start_line ASC
            LIMIT ?3
            "#,
        )
        .map_err(|e| format!("prepare find_references failed: {e}"))?;
    let rows = stmt
        .query_map(params![workspace_id, symbol_id, limit as i64], |row| {
            Ok(ReferenceHit {
                reference_id: row.get(0)?,
                file_id: row.get(1)?,
                path: row.get(2)?,
                start_line: row.get(3)?,
                start_col: row.get(4)?,
                end_line: row.get(5)?,
                end_col: row.get(6)?,
                source: row.get(7)?,
                confidence: row.get(8)?,
            })
        })
        .map_err(|e| format!("query find_references failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("read reference row failed: {e}"))?);
    }
    Ok(out)
}

pub fn get_chunk(conn: &Connection, workspace_id: &str, chunk_id: &str) -> Result<Option<ChunkLookupHit>, String> {
    index_store::ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT c.chunk_id, c.file_id, f.path, f.language, c.kind, c.start_line, c.end_line, c.text
            FROM chunks c
            JOIN files f ON f.file_id = c.file_id
            WHERE c.workspace_id = ?1 AND c.chunk_id = ?2
            LIMIT 1
            "#,
        )
        .map_err(|e| format!("prepare get_chunk failed: {e}"))?;
    let mut rows = stmt.query(params![workspace_id, chunk_id]).map_err(|e| format!("query get_chunk failed: {e}"))?;
    let Some(row) = rows.next().map_err(|e| format!("read get_chunk row failed: {e}"))? else {
        return Ok(None);
    };
    Ok(Some(ChunkLookupHit {
        chunk_id: row.get(0).map_err(|e| format!("read chunk_id failed: {e}"))?,
        file_id: row.get(1).map_err(|e| format!("read file_id failed: {e}"))?,
        path: row.get(2).map_err(|e| format!("read path failed: {e}"))?,
        language: row.get(3).map_err(|e| format!("read language failed: {e}"))?,
        kind: row.get(4).map_err(|e| format!("read kind failed: {e}"))?,
        start_line: row.get(5).map_err(|e| format!("read start_line failed: {e}"))?,
        end_line: row.get(6).map_err(|e| format!("read end_line failed: {e}"))?,
        text: row.get(7).map_err(|e| format!("read text failed: {e}"))?,
    }))
}

pub fn get_file_excerpt(
    conn: &Connection,
    workspace_id: &str,
    file_id: &str,
    start_line: i64,
    end_line: i64,
) -> Result<Option<FileExcerptHit>, String> {
    index_store::ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT file_id, path, language, content
            FROM files
            WHERE workspace_id = ?1 AND file_id = ?2
            LIMIT 1
            "#,
        )
        .map_err(|e| format!("prepare get_file_excerpt failed: {e}"))?;
    let mut rows = stmt
        .query(params![workspace_id, file_id])
        .map_err(|e| format!("query get_file_excerpt failed: {e}"))?;
    let Some(row) = rows.next().map_err(|e| format!("read get_file_excerpt row failed: {e}"))? else {
        return Ok(None);
    };
    let content: String = row.get(3).map_err(|e| format!("read content failed: {e}"))?;
    let lines: Vec<&str> = content.lines().collect();
    let start = start_line.max(1) as usize;
    let end = end_line.max(start_line).max(1) as usize;
    let excerpt = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let line = i + 1;
            line >= start && line <= end
        })
        .map(|(_, line)| *line)
        .collect::<Vec<&str>>()
        .join("\n");
    Ok(Some(FileExcerptHit {
        file_id: row.get(0).map_err(|e| format!("read file_id failed: {e}"))?,
        path: row.get(1).map_err(|e| format!("read path failed: {e}"))?,
        language: row.get(2).map_err(|e| format!("read language failed: {e}"))?,
        start_line,
        end_line,
        excerpt,
    }))
}

pub fn list_recent_index_errors(conn: &Connection, workspace_id: &str, limit: usize) -> Result<Vec<IndexErrorHit>, String> {
    index_store::ensure_schema(conn)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT path, message, created_at
            FROM index_errors
            WHERE workspace_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .map_err(|e| format!("prepare list index errors failed: {e}"))?;
    let rows = stmt
        .query_map(params![workspace_id, limit as i64], |row| {
            Ok(IndexErrorHit {
                path: row.get(0)?,
                message: row.get(1)?,
                created_at: row.get(2)?,
            })
        })
        .map_err(|e| format!("query list index errors failed: {e}"))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|e| format!("read index error row failed: {e}"))?);
    }
    Ok(out)
}
