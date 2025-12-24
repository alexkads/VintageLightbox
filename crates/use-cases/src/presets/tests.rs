//! Unit tests for Preset Use Cases
//!
//! Tests using mockall for PresetRepository mock.

use std::sync::Arc;
use domain::{
    entities::{Preset, PresetId, preset::PresetAdjustments},
    repositories::PresetRepository,
    DomainError, DomainResult,
};
use mockall::mock;
use crate::presets::{SavePresetUseCase, ListPresetsUseCase, DeletePresetUseCase};

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
    mock_repo
        .expect_save()
        .times(1)
        .returning(|_| Ok(()));

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
        PresetAdjustments { exposure: Some(0.5), ..Default::default() }
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
    
    // Should have system presets + user preset
    assert!(presets.len() >= 5); // At least 5 system presets
    
    // Check system presets exist
    let system_names: Vec<_> = presets.iter()
        .filter(|p| p.is_system)
        .map(|p| p.name.as_str())
        .collect();
    assert!(system_names.contains(&"Auto"));
    assert!(system_names.contains(&"B&W"));
    assert!(system_names.contains(&"Warm"));
    
    // Check user preset exists
    let user_presets: Vec<_> = presets.iter()
        .filter(|p| !p.is_system)
        .collect();
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
    
    // Should have at least system presets
    assert!(presets.len() >= 5);
    assert!(presets.iter().all(|p| p.is_system));
}

// ============================================
// DeletePresetUseCase Tests
// ============================================

#[tokio::test]
async fn test_delete_preset_success() {
    // Arrange
    let mut mock_repo = MockPresetRepo::new();
    mock_repo
        .expect_delete()
        .times(1)
        .returning(|_| Ok(()));

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
