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
    let adjustments = PresetAdjustments {
        exposure: Some(1.5),
        contrast: Some(1.2),
        temperature: Some(5.0),
        ..Default::default()
    };
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
    assert_eq!(found_preset.adjustments.exposure, Some(1.5));
    assert_eq!(found_preset.adjustments.contrast, Some(1.2));
    assert_eq!(found_preset.adjustments.temperature, Some(5.0));
    assert!(!found_preset.is_system);
}

#[tokio::test]
async fn test_save_system_preset() {
    // Arrange
    let repo = create_test_repository().await;
    let adjustments = PresetAdjustments {
        saturation: Some(-100.0),
        ..Default::default()
    };
    let preset = Preset::system("B&W", adjustments);
    let preset_id = preset.id;

    // Act
    repo.save(&preset).await.unwrap();
    let found = repo.find_by_id(&preset_id).await.unwrap().unwrap();

    // Assert
    assert!(found.is_system);
    assert_eq!(found.name, "B&W");
    assert_eq!(found.adjustments.saturation, Some(-100.0));
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
    let adjustments = PresetAdjustments {
        exposure: Some(0.5),
        ..Default::default()
    };
    let mut preset = Preset::user("Original".to_string(), adjustments);
    let preset_id = preset.id;

    repo.save(&preset).await.unwrap();

    // Act - Update
    preset.name = "Updated".to_string();
    preset.adjustments.exposure = Some(2.0);
    repo.save(&preset).await.unwrap(); // Upsert

    // Assert
    let found = repo.find_by_id(&preset_id).await.unwrap().unwrap();
    assert_eq!(found.name, "Updated");
    assert_eq!(found.adjustments.exposure, Some(2.0));
}

#[tokio::test]
async fn test_preset_with_all_adjustments() {
    // Arrange
    let repo = create_test_repository().await;
    let adjustments = PresetAdjustments {
        exposure: Some(1.5),
        contrast: Some(1.2),
        temperature: Some(10.0),
        tint: Some(-5.0),
        highlights: Some(50.0),
        shadows: Some(-30.0),
        whites: Some(20.0),
        blacks: Some(-10.0),
        clarity: Some(0.5),
        vibrance: Some(0.3),
        saturation: Some(-0.2),
        tone_curve_shadows: Some(5.0),
        tone_curve_darks: Some(10.0),
        tone_curve_lights: Some(-5.0),
        tone_curve_highlights: Some(-10.0),
    };
    let preset = Preset::user("Full Preset".to_string(), adjustments);
    let preset_id = preset.id;

    // Act
    repo.save(&preset).await.unwrap();
    let found = repo.find_by_id(&preset_id).await.unwrap().unwrap();

    // Assert - all fields preserved
    assert_eq!(found.adjustments.exposure, Some(1.5));
    assert_eq!(found.adjustments.contrast, Some(1.2));
    assert_eq!(found.adjustments.temperature, Some(10.0));
    assert_eq!(found.adjustments.tint, Some(-5.0));
    assert_eq!(found.adjustments.highlights, Some(50.0));
    assert_eq!(found.adjustments.shadows, Some(-30.0));
    assert_eq!(found.adjustments.whites, Some(20.0));
    assert_eq!(found.adjustments.blacks, Some(-10.0));
    assert_eq!(found.adjustments.clarity, Some(0.5));
    assert_eq!(found.adjustments.vibrance, Some(0.3));
    assert_eq!(found.adjustments.saturation, Some(-0.2));
    assert_eq!(found.adjustments.tone_curve_shadows, Some(5.0));
    assert_eq!(found.adjustments.tone_curve_darks, Some(10.0));
    assert_eq!(found.adjustments.tone_curve_lights, Some(-5.0));
    assert_eq!(found.adjustments.tone_curve_highlights, Some(-10.0));
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
