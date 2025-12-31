/// E2E Test for Crop Application in Image Processing
///
/// Verifies that crop settings are correctly applied to processed images
/// in the infrastructure layer (ImageAlgorithms::process_image).
///
/// This test ensures the fix for the crop persistence bug is working:
/// - Crop settings should be applied to processed images
/// - Crop should work in combination with color edits
/// - Processed image dimensions should match crop settings

use image::DynamicImage;
use domain::value_objects::{PhotoEdits, CropSettings};
use infrastructure::image_processing::algorithms::ImageAlgorithms;

#[test]
fn test_crop_applied_to_processed_image() {
    // Create a test image (100x100)
    let img = DynamicImage::new_rgb8(100, 100);
    
    // Create edits with crop settings (center 50x50)
    let mut edits = PhotoEdits::default();
    edits.crop_settings = Some(CropSettings::new(
        0.25, 0.25, 0.5, 0.5, // x, y, width, height (normalized)
        0, 0.0, false, false  // rotation_90, angle, flip_h, flip_v
    ));
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // Verify crop was applied
    assert_eq!(processed.width(), 50, "Processed image width should be 50 (cropped from 100)");
    assert_eq!(processed.height(), 50, "Processed image height should be 50 (cropped from 100)");
}

#[test]
fn test_crop_with_exposure_adjustment() {
    // Create a test image (200x100)
    let img = DynamicImage::new_rgb8(200, 100);
    
    // Create edits with both exposure and crop
    let mut edits = PhotoEdits::default();
    edits.exposure = 1.0; // +1 stop
    edits.crop_settings = Some(CropSettings::new(
        0.0, 0.0, 0.5, 1.0, // Left half of image
        0, 0.0, false, false
    ));
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // Verify crop was applied (should be 100x100, left half)
    assert_eq!(processed.width(), 100, "Processed image width should be 100 (left half of 200)");
    assert_eq!(processed.height(), 100, "Processed image height should remain 100");
    
    // Note: We can't easily verify exposure was applied without pixel inspection,
    // but the dimensions confirm crop was applied after color adjustments
}

#[test]
fn test_crop_with_rotation() {
    // Create a rectangular test image (100x50)
    let img = DynamicImage::new_rgb8(100, 50);
    
    // Create edits with crop and 90-degree rotation
    let mut edits = PhotoEdits::default();
    edits.crop_settings = Some(CropSettings::new(
        0.0, 0.0, 0.5, 1.0, // Left half (50x50)
        1, 0.0, false, false // 90-degree rotation
    ));
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // After crop (50x50) and 90-degree rotation, dimensions should be swapped
    assert_eq!(processed.width(), 50, "After 90° rotation, width should be 50");
    assert_eq!(processed.height(), 50, "After 90° rotation, height should be 50");
}

#[test]
fn test_crop_with_flip_horizontal() {
    // Create a test image
    let img = DynamicImage::new_rgb8(100, 100);
    
    // Create edits with crop and horizontal flip
    let mut edits = PhotoEdits::default();
    edits.crop_settings = Some(CropSettings::new(
        0.25, 0.25, 0.5, 0.5, // Center 50x50
        0, 0.0, true, false   // flip_horizontal = true
    ));
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // Verify dimensions (flip doesn't change dimensions)
    assert_eq!(processed.width(), 50);
    assert_eq!(processed.height(), 50);
}

#[test]
fn test_no_crop_returns_full_image() {
    // Create a test image
    let img = DynamicImage::new_rgb8(100, 100);
    
    // Create edits WITHOUT crop settings
    let edits = PhotoEdits::default();
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // Verify image dimensions unchanged
    assert_eq!(processed.width(), 100, "Without crop, width should remain 100");
    assert_eq!(processed.height(), 100, "Without crop, height should remain 100");
}

#[test]
fn test_crop_with_all_adjustments() {
    // Create a test image
    let img = DynamicImage::new_rgb8(200, 200);
    
    // Create edits with multiple adjustments
    let mut edits = PhotoEdits::default();
    edits.exposure = 0.5;
    edits.contrast = 1.2;
    edits.saturation = 0.3;
    edits.crop_settings = Some(CropSettings::new(
        0.25, 0.25, 0.5, 0.5, // Center quarter (100x100)
        0, 0.0, false, false
    ));
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // Verify crop was applied after all color adjustments
    assert_eq!(processed.width(), 100, "Final image should be cropped to 100x100");
    assert_eq!(processed.height(), 100, "Final image should be cropped to 100x100");
}

#[test]
fn test_crop_edge_case_full_image() {
    // Create a test image
    let img = DynamicImage::new_rgb8(100, 100);
    
    // Create edits with crop that covers entire image
    let mut edits = PhotoEdits::default();
    edits.crop_settings = Some(CropSettings::new(
        0.0, 0.0, 1.0, 1.0, // Full image
        0, 0.0, false, false
    ));
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // Verify dimensions unchanged (full crop)
    assert_eq!(processed.width(), 100);
    assert_eq!(processed.height(), 100);
}

#[test]
fn test_crop_edge_case_minimum_size() {
    // Create a test image
    let img = DynamicImage::new_rgb8(100, 100);
    
    // Create edits with very small crop (1% of image)
    let mut edits = PhotoEdits::default();
    edits.crop_settings = Some(CropSettings::new(
        0.0, 0.0, 0.01, 0.01, // 1x1 pixel crop
        0, 0.0, false, false
    ));
    
    // Process the image
    let processed = ImageAlgorithms::process_image(&img, &edits);
    
    // Verify minimum dimensions (should be at least 1x1)
    assert!(processed.width() >= 1, "Minimum width should be 1");
    assert!(processed.height() >= 1, "Minimum height should be 1");
}
