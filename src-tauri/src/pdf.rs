use std::path::Path;

pub fn extract_text(path: &Path) -> Result<String, String> {
    pdf_extract::extract_text(path).map_err(|e| format!("pdf extract failed: {e}"))
}
