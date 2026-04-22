#[derive(Debug, Clone)]
pub struct ChunkRecord {
    pub chunk_id: String,
    pub kind: String,
    pub start_line: i64,
    pub end_line: i64,
    pub text: String,
    pub text_hash: String,
}

#[derive(Debug, Clone)]
pub struct SymbolRecord {
    pub symbol_id: String,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub signature: Option<String>,
    pub container_symbol_id: Option<String>,
    pub start_line: i64,
    pub end_line: i64,
    pub visibility: Option<String>,
    pub lsp_backed: bool,
}

fn stable_id(prefix: &str, seed: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    format!("{prefix}_{}", hex::encode(hasher.finalize()))
}

fn hash_text(text: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn chunk_by_lines(file_key: &str, content: &str, chunk_size: usize, overlap: usize) -> Vec<ChunkRecord> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut start = 0usize;
    let step = chunk_size.saturating_sub(overlap).max(1);
    while start < lines.len() {
        let end = (start + chunk_size).min(lines.len());
        let text = lines[start..end].join("\n");
        let seed = format!("{file_key}:{start}:{end}");
        out.push(ChunkRecord {
            chunk_id: stable_id("chunk", &seed),
            kind: "line_window".to_string(),
            start_line: (start + 1) as i64,
            end_line: end as i64,
            text_hash: hash_text(&text),
            text,
        });
        if end == lines.len() {
            break;
        }
        start += step;
    }
    out
}

pub fn extract_symbols(file_key: &str, language: &str, content: &str) -> Vec<SymbolRecord> {
    let mut out = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        let maybe = if language == "rust" && trimmed.starts_with("fn ") {
            Some(("function", trimmed.trim_start_matches("fn ").split('(').next().unwrap_or("").trim()))
        } else if (language == "typescript" || language == "tsx" || language == "javascript" || language == "jsx")
            && trimmed.starts_with("function ")
        {
            Some((
                "function",
                trimmed
                    .trim_start_matches("function ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim(),
            ))
        } else if language == "python" && trimmed.starts_with("def ") {
            Some(("function", trimmed.trim_start_matches("def ").split('(').next().unwrap_or("").trim()))
        } else if language == "go" && trimmed.starts_with("func ") {
            Some(("function", trimmed.trim_start_matches("func ").split('(').next().unwrap_or("").trim()))
        } else if (language == "typescript" || language == "tsx" || language == "javascript" || language == "jsx")
            && trimmed.starts_with("class ")
        {
            Some(("class", trimmed.trim_start_matches("class ").split_whitespace().next().unwrap_or("").trim()))
        } else if language == "java" && trimmed.contains(" class ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let name = parts.windows(2).find_map(|w| if w[0] == "class" { Some(w[1]) } else { None });
            name.map(|n| ("class", n))
        } else {
            None
        };
        if let Some((kind, name)) = maybe {
            if name.is_empty() {
                continue;
            }
            let seed = format!("{file_key}:{kind}:{name}:{}", idx + 1);
            out.push(SymbolRecord {
                symbol_id: stable_id("sym", &seed),
                name: name.to_string(),
                qualified_name: name.to_string(),
                kind: kind.to_string(),
                signature: Some(trimmed.to_string()),
                container_symbol_id: None,
                start_line: (idx + 1) as i64,
                end_line: (idx + 1) as i64,
                visibility: None,
                lsp_backed: false,
            });
        }
    }
    out
}
