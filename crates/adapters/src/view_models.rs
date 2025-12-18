use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoViewModel {
    pub id: String,
    pub path: String,
    pub name: String,
    pub thumbnail_path: Option<String>,
}
