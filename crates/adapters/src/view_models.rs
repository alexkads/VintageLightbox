use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotoViewModel {
    pub id: String,
    pub path: String,
    pub name: String,
    // Future: thumbnail_path, rating, etc.
}
