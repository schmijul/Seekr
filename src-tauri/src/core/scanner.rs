use ignore::WalkBuilder;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_EXCLUDES: [&str; 11] = [
    ".git",
    "node_modules",
    "dist",
    "build",
    "target",
    ".next",
    "coverage",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
];

#[derive(Debug, Clone)]
pub struct FileCandidate {
    pub path: PathBuf,
    pub rel_path: String,
    pub language: String,
    pub size_bytes: i64,
    pub mtime_ns: i64,
    pub content_hash: String,
    pub content: String,
}

pub fn extension_to_language(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    match ext.as_str() {
        "c" => Some("c"),
        "h" => Some("c-header"),
        "cpp" | "cc" | "cxx" => Some("cpp"),
        "hpp" | "hh" | "hxx" => Some("cpp-header"),
        "rs" => Some("rust"),
        "ts" => Some("typescript"),
        "tsx" => Some("tsx"),
        "js" => Some("javascript"),
        "jsx" => Some("jsx"),
        "py" => Some("python"),
        "go" => Some("go"),
        "java" => Some("java"),
        "kt" => Some("kotlin"),
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        "md" => Some("markdown"),
        _ => None,
    }
}

fn has_nul(bytes: &[u8]) -> bool {
    bytes.iter().any(|b| *b == 0)
}

fn hash_content(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn scan_workspace(root: &str) -> Result<Vec<FileCandidate>, String> {
    let mut builder = WalkBuilder::new(root);
    builder.hidden(false);
    builder.git_ignore(true);
    builder.git_exclude(true);
    builder.require_git(false);
    let mut out = Vec::new();
    for entry in builder.build() {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        let path = entry.path().to_path_buf();
        if path.components().any(|c| {
            let seg = c.as_os_str().to_string_lossy();
            DEFAULT_EXCLUDES.iter().any(|x| *x == seg)
        }) {
            continue;
        }
        let Some(language) = extension_to_language(&path) else {
            continue;
        };
        let rel_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.len() > 2 * 1024 * 1024 {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if has_nul(&bytes) {
            continue;
        }
        let content = match String::from_utf8(bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mtime_ns = meta
            .modified()
            .ok()
            .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        out.push(FileCandidate {
            path,
            rel_path,
            language: language.to_string(),
            size_bytes: meta.len() as i64,
            mtime_ns,
            content_hash: hash_content(&content),
            content,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::extension_to_language;
    use std::path::Path;

    #[test]
    fn supports_c_cpp_and_headers() {
        assert_eq!(extension_to_language(Path::new("main.c")), Some("c"));
        assert_eq!(extension_to_language(Path::new("main.cpp")), Some("cpp"));
        assert_eq!(extension_to_language(Path::new("main.cc")), Some("cpp"));
        assert_eq!(extension_to_language(Path::new("main.cxx")), Some("cpp"));
        assert_eq!(extension_to_language(Path::new("main.h")), Some("c-header"));
        assert_eq!(extension_to_language(Path::new("main.hpp")), Some("cpp-header"));
        assert_eq!(extension_to_language(Path::new("main.hh")), Some("cpp-header"));
        assert_eq!(extension_to_language(Path::new("main.hxx")), Some("cpp-header"));
    }
}
