use serde::Serialize;
use crate::core::ingest::{SnapshotReport, WorkspaceStatus};
use crate::core::query::{ChunkLookupHit, FileExcerptHit, IndexErrorHit, ReferenceHit, SymbolLookupHit};
use crate::core::text_index::SearchHit;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub ok: bool,
    pub db_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReindexStatus {
    pub indexed: usize,
    pub removed: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub title: String,
    pub path: String,
    pub snippet: String,
    pub file_type: String,
    pub modified_ts: i64,
}

pub type SnapshotStatus = SnapshotReport;
pub type WorkspaceHealth = WorkspaceStatus;
pub type TextSearchHit = SearchHit;
pub type SymbolSearchHit = SearchHit;
pub type DefinitionHit = SymbolLookupHit;
pub type ReferenceSearchHit = ReferenceHit;
pub type ChunkHit = ChunkLookupHit;
pub type FileExcerpt = FileExcerptHit;
pub type IndexError = IndexErrorHit;
