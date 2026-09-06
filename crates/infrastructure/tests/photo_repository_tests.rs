//! Integration tests for PhotoRepository
//!
//! Testes de integração usando banco SQLite em memória.

use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::{ColorLabel, FilePath, PhotoMetadata, Rating},
};
use infrastructure::{create_pool, run_migrations, PhotoRepositoryImpl};

/// Helper para criar um repository de teste com banco em memória
async fn create_test_repository() -> PhotoRepositoryImpl {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("Failed to create pool");

    run_migrations(&pool)
        .await
        .expect("Failed to run migrations");

    PhotoRepositoryImpl::new(pool)
}

#[tokio::test]
async fn test_save_and_find_photo() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test.jpg").unwrap();
    let photo = Photo::new(file_path.clone());
    let photo_id = photo.id();

    // Act - Save
    let save_result = repo.save(&photo).await;
    assert!(save_result.is_ok());

    // Act - Find
    let found = repo.find_by_id(&photo_id).await.unwrap();

    // Assert
    assert!(found.is_some());
    let found_photo = found.unwrap();
    assert_eq!(found_photo.id(), photo_id);
    assert_eq!(found_photo.file_path(), &file_path);
}

#[tokio::test]
async fn test_update_photo_rating() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test.jpg").unwrap();
    let mut photo = Photo::new(file_path);
    let photo_id = photo.id();

    // Save initial photo
    repo.save(&photo).await.unwrap();

    // Act - Update rating
    photo.rate(Rating::new(5).unwrap()).unwrap();
    let update_result = repo.update(&photo).await;

    // Assert
    assert!(update_result.is_ok());

    // Verify update
    let found = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(found.rating(), Some(Rating::new(5).unwrap()));
}

#[tokio::test]
async fn test_delete_photo() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test.jpg").unwrap();
    let photo = Photo::new(file_path);
    let photo_id = photo.id();

    // Save photo
    repo.save(&photo).await.unwrap();

    // Act - Delete
    let delete_result = repo.delete(&photo_id).await;

    // Assert
    assert!(delete_result.is_ok());

    // Verify deletion
    let found = repo.find_by_id(&photo_id).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
async fn test_find_all_photos() {
    // Arrange
    let repo = create_test_repository().await;

    let photo1 = Photo::new(FilePath::new("/photos/photo1.jpg").unwrap());
    let photo2 = Photo::new(FilePath::new("/photos/photo2.jpg").unwrap());
    let photo3 = Photo::new(FilePath::new("/photos/photo3.jpg").unwrap());

    repo.save(&photo1).await.unwrap();
    repo.save(&photo2).await.unwrap();
    repo.save(&photo3).await.unwrap();

    // Act
    let all_photos = repo.find_all().await.unwrap();

    // Assert
    assert_eq!(all_photos.len(), 3);
}

#[tokio::test]
async fn test_photo_not_found() {
    // Arrange
    let repo = create_test_repository().await;
    let photo_id = domain::value_objects::PhotoId::new();

    // Act
    let found = repo.find_by_id(&photo_id).await.unwrap();

    // Assert
    assert!(found.is_none());
}

#[tokio::test]
async fn test_exists_photo() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test.jpg").unwrap();
    let photo = Photo::new(file_path);
    let photo_id = photo.id();

    // Photo doesn't exist yet
    let exists_before = repo.exists(&photo_id).await.unwrap();
    assert!(!exists_before);

    // Save photo
    repo.save(&photo).await.unwrap();

    // Act
    let exists_after = repo.exists(&photo_id).await.unwrap();

    // Assert
    assert!(exists_after);
}

#[tokio::test]
async fn test_update_photo_color_label() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test.jpg").unwrap();
    let mut photo = Photo::new(file_path);
    let photo_id = photo.id();

    repo.save(&photo).await.unwrap();

    // Act - Set color label
    photo.set_color_label(ColorLabel::Red);
    repo.update(&photo).await.unwrap();

    // Assert
    let found = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(found.color_label(), Some(ColorLabel::Red));
}

#[tokio::test]
async fn test_update_nonexistent_photo() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test.jpg").unwrap();
    let photo = Photo::new(file_path);

    // Act - Try to update photo that doesn't exist
    let result = repo.update(&photo).await;

    // Assert
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_nonexistent_photo() {
    // Arrange
    let repo = create_test_repository().await;
    let photo_id = domain::value_objects::PhotoId::new();

    // Act
    let result = repo.delete(&photo_id).await;

    // Assert
    assert!(result.is_err());
}

#[tokio::test]
async fn test_save_and_find_photo_with_metadata() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test_meta.jpg").unwrap();
    let mut photo = Photo::new(file_path);
    let photo_id = photo.id();

    // Create metadata
    let metadata = PhotoMetadata {
        camera_make: Some("Canon".to_string()),
        camera_model: Some("EOS R5".to_string()),
        date_time: Some("2023-12-17 12:00:00".to_string()),
        iso: Some(100),
        aperture: Some(2.8),
        shutter_speed: Some("1/1000".to_string()),
        focal_length: Some(50.0),
        width: Some(8192),
        height: Some(5464),
    };

    photo.set_metadata(metadata.clone());

    // Act - Save
    repo.save(&photo).await.unwrap();

    // Act - Find
    let found = repo.find_by_id(&photo_id).await.unwrap().unwrap();

    // Assert
    assert!(found.metadata().is_some());
    let found_metadata = found.metadata().unwrap();

    // Check key fields
    assert_eq!(found_metadata.camera_make, Some("Canon".to_string()));
    assert_eq!(found_metadata.camera_model, Some("EOS R5".to_string()));
    assert_eq!(found_metadata.iso, Some(100));
    assert_eq!(found_metadata.width, Some(8192));
}

#[tokio::test]
async fn test_save_and_find_photo_with_edits() {
    // Arrange
    let repo = create_test_repository().await;
    let file_path = FilePath::new("/photos/test_edits.jpg").unwrap();
    let mut photo = Photo::new(file_path);
    let photo_id = photo.id();

    // Set edits
    photo
        .set_edits(
            Some(1.5),
            Some(0.8),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // HSL (Sat)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // HSL (Hue)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // HSL (Lum)
            None,
            None,
            None, // Lens
            None,
            None, // NR
            None,
            None, // Sharpening
            None,
            None,
            None,
            None,
            None, // Tonalização
            None,
            None, // Grão
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // Crop
        )
        .unwrap();

    // Act - Save
    repo.save(&photo).await.unwrap();

    // Act - Find
    let found = repo.find_by_id(&photo_id).await.unwrap().unwrap();

    // Assert
    assert!(found.is_edited());
    assert_eq!(found.edit_exposure(), Some(1.5));
    assert_eq!(found.edit_contrast(), Some(0.8));

    // Act - Update (modify edits)
    let mut found_mut = found;
    found_mut
        .set_edits(
            Some(-0.5),
            Some(1.2),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // HSL (Sat)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // HSL (Hue)
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // HSL (Lum)
            None,
            None,
            None, // Lens
            None,
            None, // NR
            None,
            None, // Sharpening
            None,
            None,
            None,
            None,
            None, // Tonalização
            None,
            None, // Grão
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None, // Crop
        )
        .unwrap();
    repo.update(&found_mut).await.unwrap();

    // Verify Update
    let updated = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(updated.edit_exposure(), Some(-0.5));
    assert_eq!(updated.edit_contrast(), Some(1.2));
}

/// 🚨 O id da foto no site sobrevive ao banco — na gravação **e** na alteração.
///
/// É o que permite desfazer: zerar a classificação tira a foto do storage, e sem
/// o id remoto o app só saberia subir. O defeito que este teste pega é mudo — um
/// `?` a menos no `INSERT` ou um `.bind` fora de ordem grava a coluna errada, e o
/// sintoma aparece semanas depois, quando alguém tenta remover uma foto e o site
/// responde que ela não existe.
#[tokio::test]
async fn o_id_no_site_sobrevive_a_gravacao_e_a_alteracao() {
    let repo = create_test_repository().await;
    let mut photo = Photo::new(FilePath::new("/photos/ensaio.jpg").unwrap());
    let id = photo.id();

    // Nasce só local: no fluxo do dono é a classificação que autoriza a subir.
    assert!(!photo.esta_no_site());
    repo.save(&photo).await.unwrap();
    assert_eq!(
        repo.find_by_id(&id).await.unwrap().unwrap().id_no_site(),
        None
    );

    // Subiu: o site devolveu o id dela.
    photo.definir_id_no_site(Some("foto-remota-1".into()));
    repo.update(&photo).await.unwrap();
    let lida = repo.find_by_id(&id).await.unwrap().unwrap();
    assert_eq!(lida.id_no_site(), Some("foto-remota-1"));
    assert!(lida.esta_no_site());

    // Zerou a classificação: saiu do storage e volta a ser só local.
    photo.definir_id_no_site(None);
    repo.update(&photo).await.unwrap();
    assert_eq!(
        repo.find_by_id(&id).await.unwrap().unwrap().id_no_site(),
        None
    );
}

/// E o `INSERT` grava o id quando a foto já nasce sabendo dele.
#[tokio::test]
async fn uma_foto_que_ja_nasce_no_site_e_gravada_com_o_id() {
    let repo = create_test_repository().await;
    let mut photo = Photo::new(FilePath::new("/photos/outra.jpg").unwrap());
    photo.definir_id_no_site(Some("foto-remota-2".into()));
    let id = photo.id();

    repo.save(&photo).await.unwrap();

    assert_eq!(
        repo.find_by_id(&id).await.unwrap().unwrap().id_no_site(),
        Some("foto-remota-2")
    );
}
