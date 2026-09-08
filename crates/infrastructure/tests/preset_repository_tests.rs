//! Integration tests for PresetRepository
//!
//! Tests using in-memory SQLite database.

use domain::{
    entities::{preset::PresetAdjustments, Preset},
    repositories::PresetRepository,
};
use infrastructure::{create_pool, run_migrations, SqlitePresetRepository};

/// Helper to create a test repository with in-memory database
async fn create_test_repository() -> SqlitePresetRepository {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    SqlitePresetRepository::new(pool)
}

#[tokio::test]
async fn test_save_and_find_preset() {
    // Arrange
    let repo = create_test_repository().await;
    let adjustments = PresetAdjustments::vazia()
        .com("exposure", 1.5)
        .com("contrast", 1.2)
        .com("temperature", 5.0);
    let preset = Preset::user("My Preset".to_string(), adjustments);
    let preset_id = preset.id;

    // Act - Save
    let save_result = repo.save(&preset).await;
    assert!(save_result.is_ok());

    // Act - Find
    let found = repo.find_by_id(&preset_id).await.unwrap();

    // Assert
    assert!(found.is_some());
    let found_preset = found.unwrap();
    assert_eq!(found_preset.id, preset_id);
    assert_eq!(found_preset.name, "My Preset");
    assert_eq!(found_preset.adjustments.get("exposure"), Some(1.5));
    assert_eq!(found_preset.adjustments.get("contrast"), Some(1.2));
    assert_eq!(found_preset.adjustments.get("temperature"), Some(5.0));
    assert!(!found_preset.is_system);
}

#[tokio::test]
async fn test_save_system_preset() {
    // Arrange
    let repo = create_test_repository().await;
    let adjustments = PresetAdjustments::vazia().com("saturation", -1.0);
    let preset = Preset::system("B&W", adjustments);
    let preset_id = preset.id;

    // Act
    repo.save(&preset).await.unwrap();
    let found = repo.find_by_id(&preset_id).await.unwrap().unwrap();

    // Assert
    assert!(found.is_system);
    assert_eq!(found.name, "B&W");
    assert_eq!(found.adjustments.get("saturation"), Some(-1.0));
}

#[tokio::test]
async fn test_find_all_presets() {
    // Arrange
    let repo = create_test_repository().await;

    let preset1 = Preset::user("Preset A".to_string(), PresetAdjustments::default());
    let preset2 = Preset::user("Preset B".to_string(), PresetAdjustments::default());
    let preset3 = Preset::system("System C", PresetAdjustments::default());

    repo.save(&preset1).await.unwrap();
    repo.save(&preset2).await.unwrap();
    repo.save(&preset3).await.unwrap();

    // Act
    let all_presets = repo.find_all().await.unwrap();

    // Assert
    assert_eq!(all_presets.len(), 3);

    // Verify sorted by name
    assert_eq!(all_presets[0].name, "Preset A");
    assert_eq!(all_presets[1].name, "Preset B");
    assert_eq!(all_presets[2].name, "System C");
}

#[tokio::test]
async fn test_delete_preset() {
    // Arrange
    let repo = create_test_repository().await;
    let preset = Preset::user("To Delete".to_string(), PresetAdjustments::default());
    let preset_id = preset.id;

    repo.save(&preset).await.unwrap();

    // Act
    let delete_result = repo.delete(&preset_id).await;

    // Assert
    assert!(delete_result.is_ok());
    let found = repo.find_by_id(&preset_id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_update_preset() {
    // Arrange
    let repo = create_test_repository().await;
    let adjustments = PresetAdjustments::vazia().com("exposure", 0.5);
    let mut preset = Preset::user("Original".to_string(), adjustments);
    let preset_id = preset.id;

    repo.save(&preset).await.unwrap();

    // Act - Update
    preset.name = "Updated".to_string();
    preset.adjustments = preset.adjustments.clone().com("exposure", 2.0);
    repo.save(&preset).await.unwrap(); // Upsert

    // Assert
    let found = repo.find_by_id(&preset_id).await.unwrap().unwrap();
    assert_eq!(found.name, "Updated");
    assert_eq!(found.adjustments.get("exposure"), Some(2.0));
}

#[tokio::test]
async fn uma_predefinicao_atravessa_com_os_53_campos() {
    // 🚨 Eram 15 colunas, e este teste guardava exatamente as 15. Uma
    // predefinição com tonalização ou HSL era gravada **calada**, sem as duas —
    // o defeito que a migration 020 desfez, e que só se vê guardando um campo de
    // cada família e conferindo a volta.
    let repo = create_test_repository().await;
    let campos: Vec<(&str, f32)> = vec![
        ("exposure", 1.5),
        ("contrast", 1.2),
        ("temperature", 10.0),
        ("tint", -5.0),
        ("highlights", 50.0),
        ("shadows", -30.0),
        ("whites", 20.0),
        ("blacks", -10.0),
        ("clarity", 0.5),
        ("vibrance", 0.3),
        ("saturation", -0.2),
        ("tone_curve_shadows", 5.0),
        ("tone_curve_darks", 10.0),
        ("tone_curve_lights", -5.0),
        ("tone_curve_highlights", -10.0),
        ("hsl_blue_sat", 25.0),
        ("hsl_orange_hue", -12.0),
        ("hsl_green_lum", 40.0),
        ("lens_vignette_amount", -35.0),
        ("nr_luminance", 15.0),
        ("sharpen_amount", 55.0),
        ("sharpen_radius", 1.2),
        ("split_shadow_hue", 35.0),
        ("split_shadow_sat", 45.0),
        ("split_balance", -20.0),
        ("grain_amount", 30.0),
        ("grain_size", 60.0),
    ];
    let adjustments: PresetAdjustments = campos.iter().copied().collect();
    let preset = Preset::user("Full Preset".to_string(), adjustments);
    let preset_id = preset.id;

    repo.save(&preset).await.unwrap();
    let found = repo.find_by_id(&preset_id).await.unwrap().unwrap();

    assert_eq!(found.adjustments.len(), campos.len());
    for (campo, valor) in campos {
        assert_eq!(
            found.adjustments.get(campo),
            Some(valor),
            "`{campo}` não voltou do banco"
        );
    }
}

/// ⚠️ **Ausente e neutro não são a mesma coisa, e o banco tem de saber disso.**
///
/// Uma predefinição que não menciona o contraste deixa o contraste como está; a
/// que o menciona no neutro (1,0) **devolve** o contraste ao neutro. Se a ida e
/// a volta pelo banco confundissem as duas, a segunda deixaria de apagar o que
/// havia antes — e ninguém veria erro nenhum.
#[tokio::test]
async fn campo_ausente_nao_volta_como_neutro() {
    let repo = create_test_repository().await;
    let preset = Preset::user(
        "Só a exposição".to_string(),
        PresetAdjustments::vazia().com("exposure", 1.0),
    );
    let id = preset.id;

    repo.save(&preset).await.unwrap();
    let found = repo.find_by_id(&id).await.unwrap().unwrap();

    assert_eq!(found.adjustments.len(), 1);
    assert_eq!(found.adjustments.get("contrast"), None);
}

#[tokio::test]
async fn test_find_nonexistent_preset() {
    // Arrange
    let repo = create_test_repository().await;
    let fake_id = domain::entities::PresetId::default();

    // Act
    let found = repo.find_by_id(&fake_id).await.unwrap();

    // Assert
    assert!(found.is_none());
}
