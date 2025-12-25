use domain::import_source::{ImportSource, DeviceRepository};
use std::sync::Arc;

pub struct GetImportSourcesUseCase {
    device_repo: Arc<dyn DeviceRepository>,
}

impl GetImportSourcesUseCase {
    pub fn new(device_repo: Arc<dyn DeviceRepository>) -> Self {
        Self { device_repo }
    }

    pub async fn execute(&self) -> (Vec<ImportSource>, Vec<ImportSource>) {
        let devices = self.device_repo.get_mounted_devices().await;
        let history = self.device_repo.get_history().await;
        (devices, history)
    }
}
