use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
    pub root_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRecord {
    pub workspace_id: String,
    pub root_path: String,
    pub repo_type: String,
    pub head_ref: Option<String>,
    pub head_commit: Option<String>,
    pub index_state: String,
    pub last_snapshot_at: Option<i64>,
    pub last_incremental_at: Option<i64>,
    pub schema_version: i64,
}

pub fn workspace_id_for_root(root_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(root_path.as_bytes());
    hex::encode(hasher.finalize())
}

fn run_git(root: &str, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

pub fn detect_repo_meta(root_path: &str) -> (String, Option<String>, Option<String>) {
    if !Path::new(root_path).join(".git").exists() {
        return ("filesystem".to_string(), None, None);
    }
    let head_ref = run_git(root_path, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let head_commit = run_git(root_path, &["rev-parse", "HEAD"]);
    ("git".to_string(), head_ref, head_commit)
}

pub fn now_ts() -> i64 {
    Utc::now().timestamp()
}
