// PresetController - Adapters Layer
// Orchestrates preset operations between UI and Use Cases

use domain::entities::{preset::PresetAdjustments, Preset, PresetId};
use std::sync::Arc;
use use_cases::presets::{
    DeletePresetUseCase, ListPresetsUseCase, RenamePresetUseCase, SavePresetUseCase,
};

pub struct PresetController {
    list_presets_use_case: Arc<ListPresetsUseCase>,
    save_preset_use_case: Arc<SavePresetUseCase>,
    rename_preset_use_case: Arc<RenamePresetUseCase>,
    delete_preset_use_case: Arc<DeletePresetUseCase>,
}

impl PresetController {
    pub fn new(
        list_presets_use_case: Arc<ListPresetsUseCase>,
        save_preset_use_case: Arc<SavePresetUseCase>,
        rename_preset_use_case: Arc<RenamePresetUseCase>,
        delete_preset_use_case: Arc<DeletePresetUseCase>,
    ) -> Self {
        Self {
            list_presets_use_case,
            save_preset_use_case,
            rename_preset_use_case,
            delete_preset_use_case,
        }
    }

    /// List all presets (system + user)
    pub async fn list_presets(&self) -> Result<Vec<Preset>, String> {
        self.list_presets_use_case
            .execute()
            .await
            .map_err(|e| e.to_string())
    }

    /// Save a new user preset
    pub async fn save_preset(
        &self,
        name: String,
        adjustments: PresetAdjustments,
    ) -> Result<Preset, String> {
        self.save_preset_use_case
            .execute(name, adjustments)
            .await
            .map_err(|e| e.to_string())
    }

    /// Troca o nome de uma predefinição do fotógrafo
    pub async fn rename_preset(&self, id: &PresetId, nome: String) -> Result<(), String> {
        self.rename_preset_use_case
            .execute(id, nome)
            .await
            .map_err(|e| e.to_string())
    }

    /// Delete a preset by ID
    pub async fn delete_preset(&self, id: &PresetId) -> Result<(), String> {
        self.delete_preset_use_case
            .execute(id)
            .await
            .map_err(|e| e.to_string())
    }
}
