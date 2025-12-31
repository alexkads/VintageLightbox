// PresetController - Adapters Layer
// Orchestrates preset operations between UI and Use Cases

use std::sync::Arc;
use domain::entities::{Preset, PresetId, preset::PresetAdjustments};
use domain::value_objects::PhotoEdits;
use use_cases::presets::{ListPresetsUseCase, SavePresetUseCase, DeletePresetUseCase};

pub struct PresetController {
    list_presets_use_case: Arc<ListPresetsUseCase>,
    save_preset_use_case: Arc<SavePresetUseCase>,
    delete_preset_use_case: Arc<DeletePresetUseCase>,
}

impl PresetController {
    pub fn new(
        list_presets_use_case: Arc<ListPresetsUseCase>,
        save_preset_use_case: Arc<SavePresetUseCase>,
        delete_preset_use_case: Arc<DeletePresetUseCase>,
    ) -> Self {
        Self {
            list_presets_use_case,
            save_preset_use_case,
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

    /// Convenience: build adjustments from PhotoEdits (UI-friendly)
    pub async fn save_preset_from_edits(
        &self,
        name: String,
        edits: PhotoEdits,
    ) -> Result<Preset, String> {
        let adjustments = PresetAdjustments {
            exposure: Some(edits.exposure),
            contrast: Some(edits.contrast),
            temperature: Some(edits.temperature),
            tint: Some(edits.tint),
            highlights: Some(edits.highlights),
            shadows: Some(edits.shadows),
            whites: Some(edits.whites),
            blacks: Some(edits.blacks),
            clarity: Some(edits.clarity),
            vibrance: Some(edits.vibrance),
            saturation: Some(edits.saturation),
            tone_curve_shadows: Some(edits.tone_curve_shadows),
            tone_curve_darks: Some(edits.tone_curve_darks),
            tone_curve_lights: Some(edits.tone_curve_lights),
            tone_curve_highlights: Some(edits.tone_curve_highlights),
        };

        self.save_preset(name, adjustments).await
    }

    /// Delete a preset by ID
    pub async fn delete_preset(&self, id: &PresetId) -> Result<(), String> {
        self.delete_preset_use_case
            .execute(id)
            .await
            .map_err(|e| e.to_string())
    }
}
