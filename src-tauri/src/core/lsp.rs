use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageCapability {
    pub language: String,
    pub available: bool,
    pub configured: bool,
    pub active: bool,
    pub failed: bool,
}

pub fn default_capabilities() -> Vec<LanguageCapability> {
    vec![
        LanguageCapability {
            language: "rust".to_string(),
            available: false,
            configured: false,
            active: false,
            failed: false,
        },
        LanguageCapability {
            language: "typescript".to_string(),
            available: false,
            configured: false,
            active: false,
            failed: false,
        },
    ]
}
