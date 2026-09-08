use crate::devices::history_repo::ImportHistoryRepository;
use crate::devices::DeviceService;
use async_trait::async_trait;
use domain::import_source::{DeviceRepository, ImportSource};

pub struct InfrastructureDeviceRepository {
    device_service: DeviceService,
    history_repo: ImportHistoryRepository,
}

impl Default for InfrastructureDeviceRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl InfrastructureDeviceRepository {
    pub fn new() -> Self {
        Self {
            device_service: DeviceService::new(),
            history_repo: ImportHistoryRepository::new(),
        }
    }
}

#[async_trait]
impl DeviceRepository for InfrastructureDeviceRepository {
    async fn get_mounted_devices(&self) -> Vec<ImportSource> {
        self.device_service.get_mounted_devices()
    }

    async fn get_history(&self) -> Vec<ImportSource> {
        self.history_repo.get_recent()
    }

    async fn add_to_history(&self, path: std::path::PathBuf) {
        self.history_repo.add_recent(path);
    }
}
