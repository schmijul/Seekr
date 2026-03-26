use serde::Serialize;

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
