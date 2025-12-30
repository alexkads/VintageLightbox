//! Editor State Adapter
//!
//! Adapter temporário para sincronizar EditorService (novo) com AppState (legado).
//! Permite refatoração incremental sem quebrar código existente.
//! 
//! **NOTA**: Este adapter será removido após completar a refatoração de todos os componentes.

use adapters::services::EditorService;
use adapters::view_models::PhotoEdits;
use crate::state::AppState;

/// Adapter para sincronização bidirecional entre EditorService e AppState
pub struct EditorStateAdapter;

impl EditorStateAdapter {
    /// Sincroniza edições do EditorService para AppState
    /// 
    /// Usa as edições do service (fonte de verdade) e popula os campos
    /// legados `active_*` do AppState para compatibilidade com código existente.
    pub fn sync_to_state(editor_service: &EditorService, state: &mut AppState) {
        let edits = editor_service.current_edits();
        
        // Basic adjustments
        state.active_exposure = edits.exposure;
        state.active_contrast = edits.contrast;
        state.active_temperature = edits.temperature;
        state.active_tint = edits.tint;
        state.active_highlights = edits.highlights;
        state.active_shadows = edits.shadows;
        state.active_whites = edits.whites;
        state.active_blacks = edits.blacks;
        state.active_clarity = edits.clarity;
        state.active_vibrance = edits.vibrance;
        state.active_saturation = edits.saturation;
        
        // Tone curve
        state.active_tone_curve_shadows = edits.tone_curve_shadows;
        state.active_tone_curve_darks = edits.tone_curve_darks;
        state.active_tone_curve_lights = edits.tone_curve_lights;
        state.active_tone_curve_highlights = edits.tone_curve_highlights;
        
        // HSL Saturation
        state.active_hsl_red_sat = edits.hsl_red_sat;
        state.active_hsl_orange_sat = edits.hsl_orange_sat;
        state.active_hsl_yellow_sat = edits.hsl_yellow_sat;
        state.active_hsl_green_sat = edits.hsl_green_sat;
        state.active_hsl_aqua_sat = edits.hsl_aqua_sat;
        state.active_hsl_blue_sat = edits.hsl_blue_sat;
        state.active_hsl_purple_sat = edits.hsl_purple_sat;
        state.active_hsl_magenta_sat = edits.hsl_magenta_sat;
        
        // HSL Hue
        state.active_hsl_red_hue = edits.hsl_red_hue;
        state.active_hsl_orange_hue = edits.hsl_orange_hue;
        state.active_hsl_yellow_hue = edits.hsl_yellow_hue;
        state.active_hsl_green_hue = edits.hsl_green_hue;
        state.active_hsl_aqua_hue = edits.hsl_aqua_hue;
        state.active_hsl_blue_hue = edits.hsl_blue_hue;
        state.active_hsl_purple_hue = edits.hsl_purple_hue;
        state.active_hsl_magenta_hue = edits.hsl_magenta_hue;
        
        // HSL Luminance
        state.active_hsl_red_lum = edits.hsl_red_lum;
        state.active_hsl_orange_lum = edits.hsl_orange_lum;
        state.active_hsl_yellow_lum = edits.hsl_yellow_lum;
        state.active_hsl_green_lum = edits.hsl_green_lum;
        state.active_hsl_aqua_lum = edits.hsl_aqua_lum;
        state.active_hsl_blue_lum = edits.hsl_blue_lum;
        state.active_hsl_purple_lum = edits.hsl_purple_lum;
        state.active_hsl_magenta_lum = edits.hsl_magenta_lum;
        
        // Lens
        state.active_lens_distortion = edits.lens_distortion;
        state.active_lens_vignette_amount = edits.lens_vignette_amount;
        state.active_lens_vignette_midpoint = edits.lens_vignette_midpoint;
        
        // Noise Reduction
        state.active_nr_luminance = edits.nr_luminance;
        state.active_nr_color = edits.nr_color;
        
        // Sharpening
        state.active_sharpen_amount = edits.sharpen_amount;
        state.active_sharpen_radius = edits.sharpen_radius;
        
        // Crop settings (if any)
        if let Some(crop) = &edits.crop_settings {
            state.crop_settings = Some(crop.clone());
        }
    }
    
    /// Sincroniza edições do AppState para EditorService
    /// 
    /// Coleta os valores dos campos legados `active_*` e atualiza o service.
    /// Retorna erro se não houver sessão ativa.
    pub fn sync_from_state(state: &AppState, editor_service: &mut EditorService) -> Result<(), String> {
        let mut edits = PhotoEdits::default();
        
        // Basic adjustments
        edits.exposure = state.active_exposure;
        edits.contrast = state.active_contrast;
        edits.temperature = state.active_temperature;
        edits.tint = state.active_tint;
        edits.highlights = state.active_highlights;
        edits.shadows = state.active_shadows;
        edits.whites = state.active_whites;
        edits.blacks = state.active_blacks;
        edits.clarity = state.active_clarity;
        edits.vibrance = state.active_vibrance;
        edits.saturation = state.active_saturation;
        
        // Tone curve
        edits.tone_curve_shadows = state.active_tone_curve_shadows;
        edits.tone_curve_darks = state.active_tone_curve_darks;
        edits.tone_curve_lights = state.active_tone_curve_lights;
        edits.tone_curve_highlights = state.active_tone_curve_highlights;
        
        // HSL Saturation
        edits.hsl_red_sat = state.active_hsl_red_sat;
        edits.hsl_orange_sat = state.active_hsl_orange_sat;
        edits.hsl_yellow_sat = state.active_hsl_yellow_sat;
        edits.hsl_green_sat = state.active_hsl_green_sat;
        edits.hsl_aqua_sat = state.active_hsl_aqua_sat;
        edits.hsl_blue_sat = state.active_hsl_blue_sat;
        edits.hsl_purple_sat = state.active_hsl_purple_sat;
        edits.hsl_magenta_sat = state.active_hsl_magenta_sat;
        
        // HSL Hue
        edits.hsl_red_hue = state.active_hsl_red_hue;
        edits.hsl_orange_hue = state.active_hsl_orange_hue;
        edits.hsl_yellow_hue = state.active_hsl_yellow_hue;
        edits.hsl_green_hue = state.active_hsl_green_hue;
        edits.hsl_aqua_hue = state.active_hsl_aqua_hue;
        edits.hsl_blue_hue = state.active_hsl_blue_hue;
        edits.hsl_purple_hue = state.active_hsl_purple_hue;
        edits.hsl_magenta_hue = state.active_hsl_magenta_hue;
        
        // HSL Luminance
        edits.hsl_red_lum = state.active_hsl_red_lum;
        edits.hsl_orange_lum = state.active_hsl_orange_lum;
        edits.hsl_yellow_lum = state.active_hsl_yellow_lum;
        edits.hsl_green_lum = state.active_hsl_green_lum;
        edits.hsl_aqua_lum = state.active_hsl_aqua_lum;
        edits.hsl_blue_lum = state.active_hsl_blue_lum;
        edits.hsl_purple_lum = state.active_hsl_purple_lum;
        edits.hsl_magenta_lum = state.active_hsl_magenta_lum;
        
        // Lens
        edits.lens_distortion = state.active_lens_distortion;
        edits.lens_vignette_amount = state.active_lens_vignette_amount;
        edits.lens_vignette_midpoint = state.active_lens_vignette_midpoint;
        
        // Noise Reduction
        edits.nr_luminance = state.active_nr_luminance;
        edits.nr_color = state.active_nr_color;
        
        // Sharpening
        edits.sharpen_amount = state.active_sharpen_amount;
        edits.sharpen_radius = state.active_sharpen_radius;
        
        // Crop settings
        edits.crop_settings = state.crop_settings.clone();
        
        editor_service.update_edits(edits, "Sync from UI state")
    }
    
    /// Verifica se há mudanças entre o service e o state
    /// 
    /// Útil para debug e validação durante a transição.
    #[allow(dead_code)]
    pub fn has_changes(editor_service: &EditorService, state: &AppState) -> bool {
        let service_edits = editor_service.current_edits();
        
        service_edits.exposure != state.active_exposure ||
        service_edits.contrast != state.active_contrast ||
        service_edits.temperature != state.active_temperature
        // ... (comparar todos os campos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sync_bidirectional() {
        let mut state = AppState::new();
        let mut editor_service = EditorService::new();
        
        // Iniciar sessão
        editor_service.start_editing("photo1".to_string(), PhotoEdits::default());
        
        // Modificar state
        state.active_exposure = 1.5;
        state.active_contrast = 1.2;
        
        // Sync state -> service
        EditorStateAdapter::sync_from_state(&state, &mut editor_service).unwrap();
        
        // Verificar
        let edits = editor_service.current_edits();
        assert_eq!(edits.exposure, 1.5);
        assert_eq!(edits.contrast, 1.2);
        
        // Modificar service
        editor_service.update_field("Test", |e| {
            e.temperature = 5.0;
        }).unwrap();
        
        // Sync service -> state  
        EditorStateAdapter::sync_to_state(&editor_service, &mut state);
        
        // Verificar
        assert_eq!(state.active_temperature, 5.0);
    }
}
