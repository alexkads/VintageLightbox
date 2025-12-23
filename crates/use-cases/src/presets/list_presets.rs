use domain::entities::Preset;
use domain::entities::preset::PresetAdjustments;
use domain::repositories::PresetRepository;
use domain::DomainResult;
use std::sync::Arc;

pub struct ListPresetsUseCase {
    preset_repository: Arc<dyn PresetRepository>,
}

impl ListPresetsUseCase {
    pub fn new(preset_repository: Arc<dyn PresetRepository>) -> Self {
        Self { preset_repository }
    }

    pub async fn execute(&self) -> DomainResult<Vec<Preset>> {
        let mut user_presets = self.preset_repository.find_all().await?;
        
        // In a real scenario, system presets might be constants or loaded from elsewhere.
        // For now, let's include some hardcoded system presets for testing/demonstration if the repository returns only user presets.
        // Or we can rely on the repository to return both if we implement system presets as a special case in infrastructure.
        // For MVP, let's just return what the repository gives us, assuming system presets might be seeded or handled by the UI.
        // Wait, the plan says "Included Presets (5-10 basics)".
        // It's better to merge them here or have a SystemPresetProvider.
        // For simplicity, let's inject system presets here.
        
        let system_presets = vec![
            Preset::system("Auto", PresetAdjustments {
                exposure: Some(0.0), // Placeholder
                ..Default::default()
            }),
            Preset::system("B&W", PresetAdjustments {
                saturation: Some(-100.0),
                ..Default::default()
            }),
            Preset::system("Warm", PresetAdjustments {
                temperature: Some(15.0),
                ..Default::default()
            }),
             Preset::system("Cool", PresetAdjustments {
                temperature: Some(-15.0),
                ..Default::default()
            }),
             Preset::system("High Contrast", PresetAdjustments {
                contrast: Some(50.0),
                ..Default::default()
            }),
        ];

        let mut all_presets = system_presets;
        all_presets.append(&mut user_presets);
        
        Ok(all_presets)
    }
}
