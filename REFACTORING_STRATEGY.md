# Estratégia de Refatoração Incremental - develop_view.rs

## Contexto

O arquivo `develop_view.rs` (1080 linhas) tem ~150 ocorrências de acesso direto a `state.active_*`. Uma refatoração completa de uma vez seria arriscada e difícil de testar.

## Estratégia: Refatoração Incremental com Helper

### Fase 1: Criar EditorStateAdapter (NOVO)

Criar um adapter que sincroniza entre `AppState` (legado) e `EditorService` (novo):

```rust
// crates/ui/src/editor_state_adapter.rs

use adapters::services::EditorService;
use domain::value_objects::PhotoEdits;
use crate::state::AppState;

/// Adapter para sincronizar EditorService com AppState (legado)
/// Permite refatoração incremental sem quebrar código existente
pub struct EditorStateAdapter;

impl EditorStateAdapter {
    /// Sincroniza edições do EditorService para AppState
    pub fn sync_to_state(editor_service: &EditorService, state: &mut AppState) {
        let edits = editor_service.current_edits();
        
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
        
        state.active_tone_curve_shadows = edits.tone_curve_shadows;
        state.active_tone_curve_darks = edits.tone_curve_darks;
        state.active_tone_curve_lights = edits.tone_curve_lights;
        state.active_tone_curve_highlights = edits.tone_curve_highlights;
        
        // HSL Sat
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
        
        // HSL Lum
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
        
        // NR
        state.active_nr_luminance = edits.nr_luminance;
        state.active_nr_color = edits.nr_color;
        
        // Sharpening
        state.active_sharpen_amount = edits.sharpen_amount;
        state.active_sharpen_radius = edits.sharpen_radius;
    }
    
    /// Sincroniza edições do AppState para EditorService
    pub fn sync_from_state(state: &AppState, editor_service: &mut EditorService) -> Result<(), String> {
        let mut edits = PhotoEdits::default();
        
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
        
        edits.tone_curve_shadows = state.active_tone_curve_shadows;
        edits.tone_curve_darks = state.active_tone_curve_darks;
        edits.tone_curve_lights = state.active_tone_curve_lights;
        edits.tone_curve_highlights = state.active_tone_curve_highlights;
        
        // HSL Sat
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
        
        // HSL Lum
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
        
        // NR
        edits.nr_luminance = state.active_nr_luminance;
        edits.nr_color = state.active_nr_color;
        
        // Sharpening
        edits.sharpen_amount = state.active_sharpen_amount;
        edits.sharpen_radius = state.active_sharpen_radius;
        
        edits.crop_settings = state.crop_settings.clone();
        
        editor_service.update_edits(edits, "Sync from UI state")
    }
}
```

### Fase 2: Integrar Adapter em Pontos Estratégicos

#### 2.1 Ao carregar foto para edição
```rust
// Quando foto é selecionada em develop view
if let Some(photo_id) = state.internal_state.develop_selected_id.as_ref() {
    // Carregar edições da foto
    let photo_edits = load_photo_edits(photo_id);
    
    // Iniciar sessão de edição
    editor_service.start_editing(photo_id.clone(), photo_edits);
    
    // Sincronizar para state (para código legado)
    EditorStateAdapter::sync_to_state(&editor_service, state);
}
```

#### 2.2 Ao salvar edições
```rust
// Antes de salvar
EditorStateAdapter::sync_from_state(state, &mut editor_service)?;

// Obter edições do service (fonte de verdade)
let edits = editor_service.current_edits();

// Salvar
editor_controller.save_edits_from_photo_edits(&photo_id, edits).await?;

// Marcar como salvo
editor_service.mark_saved();
```

#### 2.3 Ao aplicar preset
```rust
// Aplicar preset via service
let preset_edits = convert_preset_to_photo_edits(&preset);
editor_service.update_edits(preset_edits, "Apply preset")?;

// Sincronizar para state
EditorStateAdapter::sync_to_state(&editor_service, state);
```

### Fase 3: Refatoração Gradual dos Sliders

Refatorar **um slider por vez**, testando após cada mudança:

#### Antes:
```rust
SliderControl::show(
    ui,
    "Exposure",
    &mut state.active_exposure,  // ❌ Acesso direto
    -5.0, 5.0, 0.01
);
```

#### Depois:
```rust
// Obter valor atual
let mut exposure = editor_service.current_edits().exposure;

// Mostrar slider
let changed = SliderControl::show(
    ui,
    "Exposure",
    &mut exposure,
    -5.0, 5.0, 0.01
);

// Atualizar service se mudou
if changed {
    editor_service.update_field("Adjust exposure", |edits| {
        edits.exposure = exposure;
    })?;
    
    // Sincronizar para state (temporário)
    EditorStateAdapter::sync_to_state(&editor_service, state);
}
```

### Fase 4: Remover Adapter Gradualmente

Após refatorar todos os componentes, o adapter se torna desnecessário:

1. Remover chamadas de `sync_to_state`
2. Remover campos `active_*` de `AppState`
3. Deletar `EditorStateAdapter`

## Benefícios da Abordagem

### ✅ Vantagens
1. **Incremental**: Refatorar um componente por vez
2. **Testável**: Cada mudança pode ser testada isoladamente
3. **Segura**: Código legado continua funcionando durante transição
4. **Reversível**: Fácil voltar atrás se necessário

### ⚠️ Desvantagens Temporárias
1. Código duplicado (adapter)
2. Sincronização bidirecional (overhead)
3. Complexidade temporária

## Plano de Execução

### Sprint 1: Setup (2h)
- [ ] Criar `EditorStateAdapter`
- [ ] Integrar em pontos de entrada/saída
- [ ] Testar sincronização bidirecional

### Sprint 2: Refatorar Sliders Básicos (3h)
- [ ] Exposure
- [ ] Contrast  
- [ ] Temperature
- [ ] Tint
- [ ] Highlights
- [ ] Shadows

### Sprint 3: Refatorar Sliders Avançados (3h)
- [ ] Tone Curve (4 sliders)
- [ ] HSL Saturation (8 sliders)
- [ ] HSL Hue (8 sliders)
- [ ] HSL Luminance (8 sliders)

### Sprint 4: Refatorar Lens/NR/Sharpen (2h)
- [ ] Lens Distortion
- [ ] Vignette
- [ ] Noise Reduction
- [ ] Sharpening

### Sprint 5: Refatorar Presets (2h)
- [ ] Aplicação de preset via service
- [ ] Salvamento de preset

### Sprint 6: Limpeza (2h)
- [ ] Remover `EditorStateAdapter`
- [ ] Remover campos `active_*`
- [ ] Simplificar `AppState`

**Total estimado: 14h**

## Alternativa: Big Bang Refactor

Se preferir refatorar tudo de uma vez (mais arriscado):

1. Criar branch separada
2. Refatorar todos os 150 pontos de acesso
3. Testar extensivamente
4. Merge quando 100% funcional

**Estimativa: 6-8h concentradas + 2h testes**  
**Risco: ALTO (muitas mudanças simultâneas)**

## Recomendação

**Usar abordagem incremental com adapter temporário.**

- Menor risco
- Mais fácil debugar
- Pode ser feito em múltiplas sessões
- Permite validação contínua

---

**Documento criado:** 30/12/2025  
**Próxima ação:** Criar `EditorStateAdapter`
