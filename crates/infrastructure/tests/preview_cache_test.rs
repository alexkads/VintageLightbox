use domain::services::{PreviewStorage, PreviewType, ThumbnailGenerator};
use domain::value_objects::{FilePath, PhotoId};
use infrastructure::cache::preview_manager::PreviewManager;
use infrastructure::ThumbnailGeneratorImpl;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
async fn test_preview_manager_cache_logic() {
    // 1. Setup
    let temp_dir = tempdir().unwrap();
    let preview_dir = temp_dir.path().join("previews");

    // Create PreviewManager (which also inits the DB)
    let preview_manager = Arc::new(PreviewManager::new_with_path(preview_dir.clone()));

    // Create a dummy image file
    let source_dir = temp_dir.path().join("source");
    std::fs::create_dir(&source_dir).unwrap();
    let img_path = source_dir.join("test.jpg");

    let mut img = image::ImageBuffer::<image::Rgb<u8>, Vec<u8>>::new(100, 100);
    for pixel in img.pixels_mut() {
        *pixel = image::Rgb([0, 255, 0]); // Green
    }
    img.save(&img_path).unwrap();
    let img_path_str = img_path.to_str().unwrap().to_string();

    let photo_id = PhotoId::new();
    let id_str = photo_id.to_string();

    // A. Verify Cache Miss
    let cached = preview_manager.get_thumbnail(&id_str);
    assert!(cached.is_none(), "Should be empty initially");

    // B. Simulate "Loader" logic: Generate & Save
    let generator = ThumbnailGeneratorImpl::new();
    let file_path = FilePath::new(&img_path_str).unwrap();

    // Run generation (async)
    let thumb_bytes = generator
        .generate(&file_path, 300)
        .await
        .expect("Failed to generate thumb");

    // Save to cache
    preview_manager
        .save(&photo_id, PreviewType::Thumbnail, &thumb_bytes)
        .expect("Failed to save to cache");

    // C. Verify Cache Hit
    let cached_hit = preview_manager.get_thumbnail(&id_str);
    assert!(cached_hit.is_some(), "Should find thumbnail in cache");

    let loaded_img = cached_hit.unwrap();
    let width = loaded_img.width();
    assert!(
        width == 100 || width == 300,
        "Width should be 100 (original) or 300 (scaled target), got {}",
        width
    );

    // D. Verify Persistence (New Manager on same DB)
    // Drop first manager to ensure DB lock is released if any (SQLite handles this, but Arc might keep connection open)
    drop(preview_manager);

    let preview_manager_2 = PreviewManager::new_with_path(preview_dir);
    let cached_hit_2 = preview_manager_2.get_thumbnail(&id_str);
    assert!(cached_hit_2.is_some(), "Should persist across instances");
}
