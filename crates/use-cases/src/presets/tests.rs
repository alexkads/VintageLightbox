//! Unit tests for Preset Use Cases
//!
//! Tests using mockall for PresetRepository mock.

use crate::presets::{
    presets_de_sistema, DeletePresetUseCase, ListPresetsUseCase, SavePresetUseCase,
};
use domain::{
    entities::{preset::PresetAdjustments, Preset, PresetId},
    repositories::PresetRepository,
    DomainError, DomainResult,
};
use mockall::mock;
use std::sync::Arc;

// Mock PresetRepository
mock! {
    pub PresetRepo {}

    #[async_trait::async_trait]
    impl PresetRepository for PresetRepo {
        async fn save(&self, preset: &Preset) -> DomainResult<()>;
        async fn find_by_id(&self, id: &PresetId) -> DomainResult<Option<Preset>>;
        async fn find_all(&self) -> DomainResult<Vec<Preset>>;
        async fn delete(&self, id: &PresetId) -> DomainResult<()>;
    }
}

// ============================================
// SavePresetUseCase Tests
// ============================================

#[tokio::test]
async fn test_save_preset_success() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo.expect_save().times(1).returning(|_| Ok(()));

    let use_case = SavePresetUseCase::new(Arc::new(mock_repo));

    let adjustments = PresetAdjustments {
        exposure: Some(1.5),
        contrast: Some(1.2),
        ..Default::default()
    };

    // Act
    let result = use_case.execute("My Preset".to_string(), adjustments).await;

    // Assert
    assert!(result.is_ok());
    let preset = result.unwrap();
    assert_eq!(preset.name, "My Preset");
    assert!(!preset.is_system);
    assert_eq!(preset.adjustments.exposure, Some(1.5));
    assert_eq!(preset.adjustments.contrast, Some(1.2));
}

#[tokio::test]
async fn test_save_preset_repository_error() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo
        .expect_save()
        .times(1)
        .returning(|_| Err(DomainError::InfrastructureError("DB error".to_string())));

    let use_case = SavePresetUseCase::new(Arc::new(mock_repo));

    let adjustments = PresetAdjustments::default();

    // Act
    let result = use_case.execute("Test".to_string(), adjustments).await;

    // Assert
    assert!(result.is_err());
}

// ============================================
// ListPresetsUseCase Tests
// ============================================

#[tokio::test]
async fn test_list_presets_returns_system_and_user() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();

    // Repository returns user presets (system presets are added by the use case)
    let user_preset = Preset::user(
        "User Preset".to_string(),
        PresetAdjustments {
            exposure: Some(0.5),
            ..Default::default()
        },
    );
    mock_repo
        .expect_find_all()
        .times(1)
        .returning(move || Ok(vec![user_preset.clone()]));

    let use_case = ListPresetsUseCase::new(Arc::new(mock_repo));

    // Act
    let result = use_case.execute().await;

    // Assert
    assert!(result.is_ok());
    let presets = result.unwrap();

    // Os quatro de sistema mais o do usuário. Eram cinco até 30/ago/2026, e o
    // que saiu foi o "Auto" — ele pedia `exposure: Some(0.0)` e não fazia nada.
    assert_eq!(presets.len(), 5);

    let system_names: Vec<_> = presets
        .iter()
        .filter(|p| p.is_system)
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(system_names, ["B&W", "Warm", "Cool", "High Contrast"]);

    // Check user preset exists
    let user_presets: Vec<_> = presets.iter().filter(|p| !p.is_system).collect();
    assert_eq!(user_presets.len(), 1);
    assert_eq!(user_presets[0].name, "User Preset");
}

#[tokio::test]
async fn test_list_presets_empty_user_presets() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo
        .expect_find_all()
        .times(1)
        .returning(|| Ok(vec![]));

    let use_case = ListPresetsUseCase::new(Arc::new(mock_repo));

    // Act
    let result = use_case.execute().await;

    // Assert
    assert!(result.is_ok());
    let presets = result.unwrap();

    // Sem nada salvo, sobram só os de sistema.
    assert_eq!(presets.len(), presets_de_sistema().len());
    assert!(presets.iter().all(|p| p.is_system));
}

/// 🚨 Preset de sistema fora da faixa dos sliders **destrói a foto**.
///
/// Este teste não existia, e por isso os quatro passaram meses pedindo números
/// de outra escala: `saturation: -100` numa escala que vai de -1 a 1,
/// `contrast: 50` num multiplicador de 0 a 2, `temperature: ±15` numa faixa de
/// -10 a 10. Nenhuma camada reclamava — o valor chega ao shader como `f32` e o
/// shader faz a conta com o que recebe.
///
/// As faixas aqui são as mesmas de `CONTROLES`
/// (`ui-gpui/src/revelacao/controles.rs`), que é quem as oferece ao arrasto.
/// Repeti-las é de propósito: o `use-cases` não pode depender da interface, e o
/// que este teste afirma é que **nenhum preset pede o que nenhum slider
/// consegue pedir**.
#[test]
fn os_presets_de_sistema_ficam_dentro_da_escala_do_motor() {
    for preset in presets_de_sistema() {
        let a = &preset.adjustments;
        let nome = &preset.name;

        for (rotulo, valor, minimo, maximo) in [
            ("exposição", a.exposure, -5.0, 5.0),
            ("contraste", a.contrast, 0.0, 2.0),
            ("temperatura", a.temperature, -10.0, 10.0),
            ("matiz", a.tint, -10.0, 10.0),
            ("altas luzes", a.highlights, -100.0, 100.0),
            ("sombras", a.shadows, -100.0, 100.0),
            ("brancos", a.whites, -100.0, 100.0),
            ("pretos", a.blacks, -100.0, 100.0),
            ("textura", a.clarity, -1.0, 1.0),
            ("intensidade", a.vibrance, -1.0, 1.0),
            ("saturação", a.saturation, -1.0, 1.0),
            ("curva/sombras", a.tone_curve_shadows, -100.0, 100.0),
            ("curva/escuros", a.tone_curve_darks, -100.0, 100.0),
            ("curva/claros", a.tone_curve_lights, -100.0, 100.0),
            ("curva/altas luzes", a.tone_curve_highlights, -100.0, 100.0),
        ] {
            if let Some(valor) = valor {
                assert!(
                    (minimo..=maximo).contains(&valor),
                    "o preset \"{nome}\" pede {rotulo} = {valor}, e o slider vai de {minimo} a {maximo}"
                );
            }
        }
    }
}

/// O que cada preset de sistema faz, em números.
///
/// Um preset que não move nada é tão defeito quanto um que move demais — foi o
/// caso do "Auto", que pedia `exposure: Some(0.0)` sobre o neutro `0.0`.
#[test]
fn cada_preset_de_sistema_move_alguma_coisa() {
    let por_nome = |nome: &str| {
        presets_de_sistema()
            .into_iter()
            .find(|p| p.name == nome)
            .unwrap_or_else(|| panic!("o preset de sistema \"{nome}\" sumiu da lista"))
    };

    // Cinza é o fator zero: `1.0 + (-1.0)`.
    assert_eq!(por_nome("B&W").adjustments.saturation, Some(-1.0));
    // O shader faz `r += t*10` em 0..255 — 15 níveis para cada lado.
    assert_eq!(por_nome("Warm").adjustments.temperature, Some(1.5));
    assert_eq!(por_nome("Cool").adjustments.temperature, Some(-1.5));
    // Multiplicador em volta de 128, com o neutro em 1,0.
    assert_eq!(por_nome("High Contrast").adjustments.contrast, Some(1.35));

    for preset in presets_de_sistema() {
        assert_ne!(
            preset.adjustments,
            PresetAdjustments::default(),
            "o preset de sistema \"{}\" não mexe em nada",
            preset.name
        );
    }
}

// ============================================
// DeletePresetUseCase Tests
// ============================================

#[tokio::test]
async fn test_delete_preset_success() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo.expect_delete().times(1).returning(|_| Ok(()));

    let use_case = DeletePresetUseCase::new(Arc::new(mock_repo));
    let preset_id = PresetId::default();

    // Act
    let result = use_case.execute(&preset_id).await;

    // Assert
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_delete_preset_repository_error() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo
        .expect_delete()
        .times(1)
        .returning(|_| Err(DomainError::InfrastructureError("Not found".to_string())));

    let use_case = DeletePresetUseCase::new(Arc::new(mock_repo));
    let preset_id = PresetId::default();

    // Act
    let result = use_case.execute(&preset_id).await;

    // Assert
    assert!(result.is_err());
}
