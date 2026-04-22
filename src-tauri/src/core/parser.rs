use tree_sitter::{Language, Node, Parser};

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

fn ts_language_for(language: &str) -> Option<Language> {
    match language {
        "rust" => Some(tree_sitter_rust::LANGUAGE.into()),
        "javascript" | "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "python" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "c" | "c-header" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" | "cpp-header" => Some(tree_sitter_cpp::LANGUAGE.into()),
        _ => None,
    }
}

fn chunk_kinds_for(language: &str) -> &'static [&'static str] {
    match language {
        "rust" => &["function_item", "impl_item", "struct_item", "enum_item", "trait_item", "mod_item"],
        "javascript" | "jsx" | "typescript" | "tsx" => &[
            "function_declaration",
            "method_definition",
            "class_declaration",
            "interface_declaration",
            "type_alias_declaration",
        ],
        "python" => &["function_definition", "class_definition"],
        "go" => &["function_declaration", "method_declaration", "type_declaration"],
        "java" => &[
            "class_declaration",
            "interface_declaration",
            "method_declaration",
            "constructor_declaration",
            "enum_declaration",
        ],
        "c" | "c-header" => &["function_definition", "struct_specifier", "enum_specifier"],
        "cpp" | "cpp-header" => &[
            "function_definition",
            "class_specifier",
            "struct_specifier",
            "namespace_definition",
            "enum_specifier",
        ],
        _ => &[],
    }
}

fn map_symbol_kind(node_kind: &str) -> Option<&'static str> {
    match node_kind {
        "function_item" | "function_definition" | "function_declaration" => Some("function"),
        "method_definition" | "method_declaration" => Some("method"),
        "constructor_declaration" => Some("constructor"),
        "class_declaration" | "class_definition" | "class_specifier" => Some("class"),
        "struct_item" | "struct_specifier" => Some("struct"),
        "enum_item" | "enum_declaration" | "enum_specifier" => Some("enum"),
        "interface_declaration" => Some("interface"),
        "trait_item" => Some("trait"),
        "mod_item" | "namespace_definition" => Some("module"),
        "type_alias_declaration" | "type_declaration" => Some("type_alias"),
        _ => None,
    }
}

fn walk_nodes<'a>(root: Node<'a>, wanted_kinds: &[&str]) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if wanted_kinds.iter().any(|k| *k == node.kind()) {
            out.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    out
}

fn clean_name(raw: &str) -> String {
    raw.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'))
        .split("::")
        .last()
        .unwrap_or(raw)
        .trim()
        .to_string()
}

fn name_from_signature(signature: &str) -> Option<String> {
    let before_paren = signature.split('(').next().unwrap_or(signature).trim();
    let token = before_paren.split_whitespace().last()?;
    let name = clean_name(token);
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

fn name_from_node(node: Node<'_>, src: &[u8]) -> Option<String> {
    if let Some(name_node) = node.child_by_field_name("name") {
        if let Ok(name_text) = name_node.utf8_text(src) {
            let name = clean_name(name_text);
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    let raw = node.utf8_text(src).ok()?;
    name_from_signature(raw)
}

fn try_ast_extract(file_key: &str, language: &str, content: &str) -> Option<(Vec<ChunkRecord>, Vec<SymbolRecord>)> {
    let ts_lang = ts_language_for(language)?;
    let wanted = chunk_kinds_for(language);
    if wanted.is_empty() {
        return None;
    }
    let mut parser = Parser::new();
    parser.set_language(&ts_lang).ok()?;
    let tree = parser.parse(content, None)?;
    let root = tree.root_node();
    let source = content.as_bytes();
    let nodes = walk_nodes(root, wanted);
    if nodes.is_empty() {
        return None;
    }

    let mut chunks = Vec::new();
    let mut symbols = Vec::new();

    for node in nodes {
        let text = match node.utf8_text(source) {
            Ok(v) => v.trim().to_string(),
            Err(_) => continue,
        };
        if text.is_empty() {
            continue;
        }
        let start_line = node.start_position().row as i64 + 1;
        let end_line = node.end_position().row as i64 + 1;
        let kind = format!("ast:{}", node.kind());
        let chunk_seed = format!("{file_key}:{kind}:{start_line}:{end_line}");
        chunks.push(ChunkRecord {
            chunk_id: stable_id("chunk", &chunk_seed),
            kind: kind.clone(),
            start_line,
            end_line,
            text_hash: hash_text(&text),
            text: text.clone(),
        });

        if let Some(mapped_kind) = map_symbol_kind(node.kind()) {
            if let Some(name) = name_from_node(node, source) {
                let sig = text.lines().next().unwrap_or("").trim().to_string();
                let symbol_seed = format!("{file_key}:{mapped_kind}:{name}:{start_line}");
                symbols.push(SymbolRecord {
                    symbol_id: stable_id("sym", &symbol_seed),
                    name: name.clone(),
                    qualified_name: name,
                    kind: mapped_kind.to_string(),
                    signature: if sig.is_empty() { None } else { Some(sig) },
                    container_symbol_id: None,
                    start_line,
                    end_line,
                    visibility: None,
                    lsp_backed: false,
                });
            }
        }
    }

    if chunks.is_empty() {
        None
    } else {
        Some((chunks, symbols))
    }
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
        let maybe: Option<(String, String)> = if language == "rust" && trimmed.starts_with("fn ") {
            Some((
                "function".to_string(),
                trimmed
                    .trim_start_matches("fn ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            ))
        } else if (language == "typescript" || language == "tsx" || language == "javascript" || language == "jsx")
            && trimmed.starts_with("function ")
        {
            Some((
                "function".to_string(),
                trimmed
                    .trim_start_matches("function ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            ))
        } else if language == "python" && trimmed.starts_with("def ") {
            Some((
                "function".to_string(),
                trimmed
                    .trim_start_matches("def ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            ))
        } else if language == "go" && trimmed.starts_with("func ") {
            Some((
                "function".to_string(),
                trimmed
                    .trim_start_matches("func ")
                    .split('(')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            ))
        } else if (language == "typescript" || language == "tsx" || language == "javascript" || language == "jsx")
            && trimmed.starts_with("class ")
        {
            Some((
                "class".to_string(),
                trimmed
                    .trim_start_matches("class ")
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string(),
            ))
        } else if language == "java" && trimmed.contains(" class ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let name = parts.windows(2).find_map(|w| if w[0] == "class" { Some(w[1]) } else { None });
            name.map(|n| ("class".to_string(), n.to_string()))
        } else if (language == "cpp" || language == "cpp-header") && trimmed.contains("class ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            let name = parts.windows(2).find_map(|w| if w[0] == "class" { Some(w[1]) } else { None });
            name.map(|n| ("class".to_string(), n.to_string()))
        } else if (language == "c" || language == "c-header" || language == "cpp" || language == "cpp-header")
            && trimmed.contains('(')
            && trimmed.ends_with('{')
        {
            name_from_signature(trimmed).map(|n| ("function".to_string(), n))
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
                name: name.clone(),
                qualified_name: name,
                kind,
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

pub fn extract_chunks_and_symbols(file_key: &str, language: &str, content: &str) -> (Vec<ChunkRecord>, Vec<SymbolRecord>, bool) {
    if let Some((chunks, symbols)) = try_ast_extract(file_key, language, content) {
        return (chunks, symbols, true);
    }
    let chunks = chunk_by_lines(file_key, content, 120, 20);
    let symbols = extract_symbols(file_key, language, content);
    (chunks, symbols, false)
}

#[cfg(test)]
mod tests {
    use super::extract_chunks_and_symbols;

    #[test]
    fn ast_extracts_rust_function() {
        let src = "pub fn hello(name: &str) -> String { format!(\"hi {}\", name) }";
        let (_chunks, symbols, ast_used) = extract_chunks_and_symbols("a.rs", "rust", src);
        assert!(ast_used);
        assert!(symbols.iter().any(|s| s.name == "hello"));
    }

    #[test]
    fn ast_extracts_cpp_function() {
        let src = "int add(int a, int b) { return a + b; }";
        let (_chunks, symbols, ast_used) = extract_chunks_and_symbols("x.cpp", "cpp", src);
        assert!(ast_used);
        assert!(symbols.iter().any(|s| s.name == "add"));
    }
}
