use domain::entities::{Preset, preset::PresetAdjustments};
use domain::repositories::PresetRepository;
use domain::DomainResult;
use std::sync::Arc;

pub struct SavePresetUseCase {
    preset_repository: Arc<dyn PresetRepository>,
}

impl SavePresetUseCase {
    pub fn new(preset_repository: Arc<dyn PresetRepository>) -> Self {
        Self { preset_repository }
    }

    pub async fn execute(
        &self,
        name: String,
        adjustments: PresetAdjustments,
    ) -> DomainResult<Preset> {
        let preset = Preset::user(name, adjustments);
        self.preset_repository.save(&preset).await?;
        Ok(preset)
    }
}
