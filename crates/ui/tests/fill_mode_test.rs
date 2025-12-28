use domain::value_objects::{CropSettings, RotationFillMode};

#[test]
fn test_with_fill_mode_method() {
    // Unit test to verify with_fill_mode works correctly
    let crop = CropSettings::default();
    assert_eq!(crop.fill_mode(), RotationFillMode::Black);
    
    let crop_white = crop.with_fill_mode(RotationFillMode::White);
    assert_eq!(crop_white.fill_mode(), RotationFillMode::White);
    
    // Original should be unchanged
    assert_eq!(crop.fill_mode(), RotationFillMode::Black);
}

#[test]
fn test_with_flip_horizontal_preserves_fill_mode() {
    let crop = CropSettings::default()
        .with_fill_mode(RotationFillMode::White);
    
    assert_eq!(crop.fill_mode(), RotationFillMode::White);
    assert_eq!(crop.flip_horizontal(), false);
    
    let flipped = crop.with_flip_horizontal(true);
    
    // Fill mode should be preserved
    assert_eq!(flipped.fill_mode(), RotationFillMode::White);
    assert_eq!(flipped.flip_horizontal(), true);
}

#[test]
fn test_with_flip_vertical_preserves_fill_mode() {
    let crop = CropSettings::default()
        .with_fill_mode(RotationFillMode::Transparent);
    
    assert_eq!(crop.fill_mode(), RotationFillMode::Transparent);
    assert_eq!(crop.flip_vertical(), false);
    
    let flipped = crop.with_flip_vertical(true);
    
    // Fill mode should be preserved
    assert_eq!(flipped.fill_mode(), RotationFillMode::Transparent);
    assert_eq!(flipped.flip_vertical(), true);
}

#[test]
fn test_with_angle_preserves_fill_mode() {
    let crop = CropSettings::default()
        .with_fill_mode(RotationFillMode::ShrinkToFit);
    
    assert_eq!(crop.fill_mode(), RotationFillMode::ShrinkToFit);
    assert_eq!(crop.angle(), 0.0);
    
    let rotated = crop.with_angle(15.0);
    
    // Fill mode should be preserved
    assert_eq!(rotated.fill_mode(), RotationFillMode::ShrinkToFit);
    assert_eq!(rotated.angle(), 15.0);
}

#[test]
fn test_with_rotation_90_preserves_fill_mode() {
    let crop = CropSettings::default()
        .with_fill_mode(RotationFillMode::Intelligent);
    
    assert_eq!(crop.fill_mode(), RotationFillMode::Intelligent);
    assert_eq!(crop.rotation_90(), 0);
    
    let rotated = crop.with_rotation_90(1);
    
    // Fill mode should be preserved
    assert_eq!(rotated.fill_mode(), RotationFillMode::Intelligent);
    assert_eq!(rotated.rotation_90(), 1);
}

#[test]
fn test_chained_operations_preserve_fill_mode() {
    let crop = CropSettings::default()
        .with_fill_mode(RotationFillMode::White)
        .with_flip_horizontal(true)
        .with_flip_vertical(true)
        .with_angle(10.0)
        .with_rotation_90(2);
    
    // Fill mode should be preserved through all operations
    assert_eq!(crop.fill_mode(), RotationFillMode::White);
    assert_eq!(crop.flip_horizontal(), true);
    assert_eq!(crop.flip_vertical(), true);
    assert_eq!(crop.angle(), 10.0);
    assert_eq!(crop.rotation_90(), 2);
}
