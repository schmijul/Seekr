use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct AppState {
    pub db_path: Mutex<Option<PathBuf>>,
}
