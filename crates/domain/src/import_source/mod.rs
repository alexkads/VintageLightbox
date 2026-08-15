use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSourceType {
    Device,
    Folder,
    History,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImportSource {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub source_type: ImportSourceType,
}

impl ImportSource {
    pub fn new_device(name: String, path: PathBuf) -> Self {
        Self {
            id: format!("dev://{}", path.display()),
            name,
            path,
            source_type: ImportSourceType::Device,
        }
    }
}
#[async_trait::async_trait]
pub trait DeviceRepository: Send + Sync {
    async fn get_mounted_devices(&self) -> Vec<ImportSource>;
    async fn get_history(&self) -> Vec<ImportSource>;
    async fn add_to_history(&self, path: std::path::PathBuf);
}
