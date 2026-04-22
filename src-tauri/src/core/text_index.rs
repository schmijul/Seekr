use serde::Serialize;
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, TantivyDocument, Value, STORED, STRING, TEXT};
use tantivy::{doc, Index};

#[derive(Debug, Clone)]
pub struct IndexedDoc {
    pub workspace_id: String,
    pub kind: String,
    pub path: String,
    pub language: String,
    pub entity_id: String,
    pub symbol_name: String,
    pub content: String,
    pub start_line: i64,
    pub end_line: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub workspace_id: String,
    pub kind: String,
    pub path: String,
    pub language: String,
    pub entity_id: String,
    pub symbol_name: String,
    pub content_excerpt: String,
    pub start_line: i64,
    pub end_line: i64,
    pub score: f32,
}

#[derive(Clone, Copy)]
struct Fields {
    workspace_id: Field,
    kind: Field,
    path: Field,
    language: Field,
    entity_id: Field,
    symbol_name: Field,
    content: Field,
    start_line: Field,
    end_line: Field,
}

fn build_schema() -> (Schema, Fields) {
    let mut sb = Schema::builder();
    let workspace_id = sb.add_text_field("workspace_id", STRING | STORED);
    let kind = sb.add_text_field("kind", STRING | STORED);
    let path = sb.add_text_field("path", TEXT | STORED);
    let language = sb.add_text_field("language", STRING | STORED);
    let entity_id = sb.add_text_field("entity_id", STRING | STORED);
    let symbol_name = sb.add_text_field("symbol_name", TEXT | STORED);
    let content = sb.add_text_field("content", TEXT | STORED);
    let start_line = sb.add_i64_field("start_line", STORED);
    let end_line = sb.add_i64_field("end_line", STORED);
    let schema = sb.build();
    (
        schema,
        Fields {
            workspace_id,
            kind,
            path,
            language,
            entity_id,
            symbol_name,
            content,
            start_line,
            end_line,
        },
    )
}

fn open_or_create_index(index_dir: &Path) -> Result<(Index, Fields), String> {
    let (schema, fields) = build_schema();
    std::fs::create_dir_all(index_dir).map_err(|e| format!("create index dir failed: {e}"))?;
    let index = match Index::open_in_dir(index_dir) {
        Ok(idx) => idx,
        Err(_) => Index::create_in_dir(index_dir, schema).map_err(|e| format!("create tantivy index failed: {e}"))?,
    };
    Ok((index, fields))
}

pub fn replace_workspace_docs(index_dir: &Path, workspace_id: &str, docs: &[IndexedDoc]) -> Result<(), String> {
    let (index, fields) = open_or_create_index(index_dir)?;
    let mut writer = index.writer(30_000_000).map_err(|e| format!("open tantivy writer failed: {e}"))?;
    writer.delete_term(tantivy::Term::from_field_text(fields.workspace_id, workspace_id));
    for d in docs {
        writer.add_document(doc!(
            fields.workspace_id => d.workspace_id.clone(),
            fields.kind => d.kind.clone(),
            fields.path => d.path.clone(),
            fields.language => d.language.clone(),
            fields.entity_id => d.entity_id.clone(),
            fields.symbol_name => d.symbol_name.clone(),
            fields.content => d.content.clone(),
            fields.start_line => d.start_line,
            fields.end_line => d.end_line,
        ))
        .map_err(|e| format!("add document to tantivy failed: {e}"))?;
    }
    writer.commit().map_err(|e| format!("commit tantivy failed: {e}"))?;
    Ok(())
}

fn search_internal(
    index_dir: &Path,
    workspace_id: &str,
    query: &str,
    kinds: &[&str],
    limit: usize,
) -> Result<Vec<SearchHit>, String> {
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let (index, fields) = open_or_create_index(index_dir)?;
    let reader = index.reader().map_err(|e| format!("open tantivy reader failed: {e}"))?;
    let searcher = reader.searcher();
    let parser = QueryParser::for_index(&index, vec![fields.symbol_name, fields.path, fields.content]);
    let parsed = parser.parse_query(query).map_err(|e| format!("parse tantivy query failed: {e}"))?;
    let top_docs = searcher
        .search(&parsed, &TopDocs::with_limit(limit))
        .map_err(|e| format!("execute tantivy search failed: {e}"))?;

    let mut out = Vec::new();
    for (score, addr) in top_docs {
        let retrieved: TantivyDocument = searcher
            .doc(addr)
            .map_err(|e| format!("load document failed: {e}"))?;
        let ws = retrieved
            .get_first(fields.workspace_id)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let kind = retrieved
            .get_first(fields.kind)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if ws != workspace_id || !kinds.iter().any(|k| *k == kind) {
            continue;
        }
        let content = retrieved
            .get_first(fields.content)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let excerpt = if content.len() > 400 {
            format!("{}...", &content[..400])
        } else {
            content
        };
        out.push(SearchHit {
            workspace_id: ws,
            kind,
            path: retrieved
                .get_first(fields.path)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            language: retrieved
                .get_first(fields.language)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            entity_id: retrieved
                .get_first(fields.entity_id)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            symbol_name: retrieved
                .get_first(fields.symbol_name)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            content_excerpt: excerpt,
            start_line: retrieved
                .get_first(fields.start_line)
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            end_line: retrieved
                .get_first(fields.end_line)
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            score,
        });
    }
    Ok(out)
}

pub fn search_text(index_dir: &Path, workspace_id: &str, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
    search_internal(index_dir, workspace_id, query, &["file", "chunk"], limit)
}

pub fn search_symbols(index_dir: &Path, workspace_id: &str, query: &str, limit: usize) -> Result<Vec<SearchHit>, String> {
    search_internal(index_dir, workspace_id, query, &["symbol"], limit)
}
