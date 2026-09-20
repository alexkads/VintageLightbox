use domain::entities::PresetId;
use domain::repositories::PresetRepository;
use domain::DomainResult;
use std::sync::Arc;

pub struct DeletePresetUseCase {
    preset_repository: Arc<dyn PresetRepository>,
}

impl DeletePresetUseCase {
    pub fn new(preset_repository: Arc<dyn PresetRepository>) -> Self {
        Self { preset_repository }
    }

    pub async fn execute(&self, id: &PresetId) -> DomainResult<()> {
        // TODO: Validate if it's a system preset before deleting?
        // The repository should probably handle or return error if trying to delete non-existent.
        // System presets are not in the DB, so delete on them would fail or do nothing.
        self.preset_repository.delete(id).await
    }
}
