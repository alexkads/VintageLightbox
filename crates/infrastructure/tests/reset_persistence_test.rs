use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::{FilePath, PhotoId},
};
use infrastructure::database::photo_repository::PhotoRepositoryImpl;
use sqlx::sqlite::SqlitePoolOptions;
use std::sync::Arc;

#[tokio::test]
async fn test_reset_persistence_with_default_values() {
    // 1. Setup In-Memory DB
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let repo = PhotoRepositoryImpl::new(pool);
    // Use trait object to ensure we test the trait interface
    // But verify if update is part of trait. If not, compilation will fail.
    // If update is not in trait, we must rely on save (upsert) or finding how UseCase does it.
    // For now, let's keep specific type to access update if needed, but intended usage is via trait
    let repo = Arc::new(repo); 

    // 2. Create Photo with existing crop edits (simulating a cropped photo)
    let mut photo = Photo::new(FilePath::new("/test/photo.jpg").unwrap());
    photo.set_edits(
        // Exposure (11) + Tone Curve (4) = 15
        None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None,

        // HSL Sat (8)
        None, None, None, None, None, None, None, None,
        // HSL Hue (8)
        None, None, None, None, None, None, None, None,
        // HSL Lum (8)
        None, None, None, None, None, None, None, None,

        // Lens (3)
        None, None, None,

        // NR (2)
        None, None,

        // Sharpen (2)
        None, None,

        // Crop (8)
        Some(0.2), Some(0.2), Some(0.6), Some(0.6), Some(0), Some(0.0), Some(false), Some(false)
    ).unwrap();

    repo.save(&photo).await.expect("Failed to save initial photo");

    // Verify initial save
    let loaded = repo.find_by_id(&photo.id()).await.unwrap().unwrap();
    assert_eq!(loaded.edit_crop_x(), Some(0.2));

    // 3. Update with "Reset" values (simulating CropSettings::default())
    // Default: x=0.0, y=0.0, w=1.0, h=1.0, rot=0
    let mut photo_to_update = loaded.clone();
    photo_to_update.set_edits(
         // Exposure (11) + Tone Curve (4) = 15
        None, None, None, None, None, None, None, None, None, None, None,
        None, None, None, None,

        // HSL Sat (8)
        None, None, None, None, None, None, None, None,
        // HSL Hue (8)
        None, None, None, None, None, None, None, None,
        // HSL Lum (8)
        None, None, None, None, None, None, None, None,

        // Lens (3)
        None, None, None,

        // NR (2)
        None, None,

        // Sharpen (2)
        None, None,

        // Crop (8) -> Reset
        Some(0.0), Some(0.0), Some(1.0), Some(1.0), Some(0), Some(0.0), Some(false), Some(false)
    ).unwrap();

    // Try calling update. If trait doesn't have it, we'll see compile error.
    // In that case, we need to check how SavePhotoEditsUseCase does it.
    repo.update(&photo_to_update).await.expect("Failed to update photo");

    // 4. Reload and verify persistence
    let reloaded = repo.find_by_id(&photo.id()).await.unwrap().unwrap();
    
    assert_eq!(reloaded.edit_crop_x(), Some(0.0), "Crop X should be 0.0 after reset");
    assert_eq!(reloaded.edit_crop_width(), Some(1.0), "Crop Width should be 1.0 after reset");
}
