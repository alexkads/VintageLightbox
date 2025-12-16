//! Photo Entity
//!
//! Entidade central do domínio representando uma fotografia.
//! Implementado usando TDD.

use crate::{
    value_objects::{ColorLabel, FilePath, PhotoId, Rating},
    DomainResult,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Entidade Photo - representa uma fotografia no catálogo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    /// Identificador único
    id: PhotoId,
    /// Caminho do arquivo
    file_path: FilePath,
    /// Classificação por estrelas (0-5)
    rating: Option<Rating>,
    /// Etiqueta de cor
    color_label: Option<ColorLabel>,
    /// Data de importação
    imported_at: DateTime<Utc>,
    /// Data de última modificação
    modified_at: DateTime<Utc>,
    /// Flag de foto editada
    is_edited: bool,
}

impl Photo {
    /// Cria uma nova foto gerando automaticamente um ID único
    pub fn new(file_path: FilePath) -> Self {
        Self::with_id(PhotoId::new(), file_path)
    }

    /// Cria uma nova foto com ID específico (para reconstrução de persistência)
    pub fn with_id(id: PhotoId, file_path: FilePath) -> Self {
        let now = Utc::now();
        Self {
            id,
            file_path,
            rating: None,
            color_label: None,
            imported_at: now,
            modified_at: now,
            is_edited: false,
        }
    }

    /// Cria uma foto para testes
    #[cfg(test)]
    pub fn new_test() -> Self {
        Self::new(FilePath::new_unchecked("/test/photo.jpg"))
    }

    /// Retorna o ID da foto
    pub fn id(&self) -> PhotoId {
        self.id
    }

    /// Retorna o caminho do arquivo
    pub fn file_path(&self) -> &FilePath {
        &self.file_path
    }

    /// Retorna o rating atual
    pub fn rating(&self) -> Option<Rating> {
        self.rating
    }

    /// Define o rating da foto
    pub fn rate(&mut self, rating: Rating) -> DomainResult<()> {
        self.rating = Some(rating);
        self.modified_at = Utc::now();
        Ok(())
    }

    /// Remove o rating da foto
    pub fn unrate(&mut self) {
        self.rating = None;
        self.modified_at = Utc::now();
    }

    /// Retorna a etiqueta de cor atual
    pub fn color_label(&self) -> Option<ColorLabel> {
        self.color_label
    }

    /// Define a etiqueta de cor
    pub fn set_color_label(&mut self, label: ColorLabel) {
        self.color_label = Some(label);
        self.modified_at = Utc::now();
    }

    /// Remove a etiqueta de cor
    pub fn remove_color_label(&mut self) {
        self.color_label = None;
        self.modified_at = Utc::now();
    }

    /// Retorna a data de importação
    pub fn imported_at(&self) -> DateTime<Utc> {
        self.imported_at
    }

    /// Retorna a data de última modificação
    pub fn modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }

    /// Verifica se a foto foi editada
    pub fn is_edited(&self) -> bool {
        self.is_edited
    }

    /// Marca a foto como editada
    pub fn mark_as_edited(&mut self) {
        self.is_edited = true;
        self.modified_at = Utc::now();
    }

    /// Marca a foto como não editada (reset)
    pub fn reset_edits(&mut self) {
        self.is_edited = false;
        self.modified_at = Utc::now();
    }

    /// Verifica se a foto tem rating
    pub fn has_rating(&self) -> bool {
        self.rating.is_some()
    }

    /// Verifica se a foto tem etiqueta de cor
    pub fn has_color_label(&self) -> bool {
        self.color_label.is_some()
    }

    /// Retorna o nome do arquivo
    pub fn file_name(&self) -> Option<&str> {
        self.file_path.file_name()
    }

    /// Retorna a extensão do arquivo
    pub fn extension(&self) -> Option<&str> {
        self.file_path.extension()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 🔴 RED -> 🟢 GREEN -> 🔵 REFACTOR

    #[test]
    fn test_photo_creation() {
        // Arrange
        let id = PhotoId::new();
        let path = FilePath::new("/test/photo.jpg").unwrap();

        // Act
        let photo = Photo::with_id(id, path.clone());

        // Assert
        assert_eq!(photo.id(), id);
        assert_eq!(photo.file_path(), &path);
        assert_eq!(photo.rating(), None);
        assert_eq!(photo.color_label(), None);
        assert!(!photo.is_edited());
    }

    #[test]
    fn test_photo_rate() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        let result = photo.rate(Rating::FIVE);

        // Assert
        assert!(result.is_ok());
        assert_eq!(photo.rating(), Some(Rating::FIVE));
        assert!(photo.has_rating());
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_rate_multiple_times() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act
        photo.rate(Rating::THREE).unwrap();
        photo.rate(Rating::FIVE).unwrap();

        // Assert
        assert_eq!(photo.rating(), Some(Rating::FIVE));
    }

    #[test]
    fn test_photo_unrate() {
        // Arrange
        let mut photo = Photo::new_test();
        photo.rate(Rating::FOUR).unwrap();

        // Act
        photo.unrate();

        // Assert
        assert_eq!(photo.rating(), None);
        assert!(!photo.has_rating());
    }

    #[test]
    fn test_photo_set_color_label() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        photo.set_color_label(ColorLabel::Red);

        // Assert
        assert_eq!(photo.color_label(), Some(ColorLabel::Red));
        assert!(photo.has_color_label());
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_remove_color_label() {
        // Arrange
        let mut photo = Photo::new_test();
        photo.set_color_label(ColorLabel::Blue);

        // Act
        photo.remove_color_label();

        // Assert
        assert_eq!(photo.color_label(), None);
        assert!(!photo.has_color_label());
    }

    #[test]
    fn test_photo_mark_as_edited() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        photo.mark_as_edited();

        // Assert
        assert!(photo.is_edited());
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_reset_edits() {
        // Arrange
        let mut photo = Photo::new_test();
        photo.mark_as_edited();

        // Act
        photo.reset_edits();

        // Assert
        assert!(!photo.is_edited());
    }

    #[test]
    fn test_photo_file_name() {
        // Arrange
        let photo = Photo::new(
            FilePath::new("/path/to/my_photo.jpg").unwrap(),
        );

        // Act
        let name = photo.file_name();

        // Assert
        assert_eq!(name, Some("my_photo.jpg"));
    }

    #[test]
    fn test_photo_extension() {
        // Arrange
        let photo1 = Photo::new(
            FilePath::new("/path/photo.jpg").unwrap(),
        );
        let photo2 = Photo::new(
            FilePath::new("/path/photo.RAW").unwrap(),
        );

        // Assert
        assert_eq!(photo1.extension(), Some("jpg"));
        assert_eq!(photo2.extension(), Some("RAW"));
    }

    #[test]
    fn test_photo_imported_and_modified_dates() {
        // Arrange & Act
        let photo = Photo::new_test();

        // Assert
        assert!(photo.imported_at() <= Utc::now());
        assert!(photo.modified_at() <= Utc::now());
        assert_eq!(photo.imported_at(), photo.modified_at());
    }

    #[test]
    fn test_photo_modification_updates_date() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        photo.rate(Rating::THREE).unwrap();

        // Assert
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_equality() {
        // Arrange
        let id = PhotoId::new();
        let path = FilePath::new("/test/photo.jpg").unwrap();
        
        let photo1 = Photo::with_id(id, path.clone());
        let photo2 = Photo::with_id(id, path);

        // Assert
        assert_eq!(photo1, photo2);
    }

    #[test]
    fn test_photo_clone() {
        // Arrange
        let mut photo1 = Photo::new_test();
        photo1.rate(Rating::FIVE).unwrap();
        photo1.set_color_label(ColorLabel::Green);

        // Act
        let photo2 = photo1.clone();

        // Assert
        assert_eq!(photo1, photo2);
        assert_eq!(photo2.rating(), Some(Rating::FIVE));
        assert_eq!(photo2.color_label(), Some(ColorLabel::Green));
    }
}

#[cfg(test)]
mod business_logic_tests {
    use super::*;

    #[test]
    fn test_rating_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act & Assert - Workflow completo de rating
        assert!(!photo.has_rating());
        
        photo.rate(Rating::THREE).unwrap();
        assert!(photo.has_rating());
        assert_eq!(photo.rating(), Some(Rating::THREE));

        photo.rate(Rating::FIVE).unwrap();
        assert_eq!(photo.rating(), Some(Rating::FIVE));

        photo.unrate();
        assert!(!photo.has_rating());
    }

    #[test]
    fn test_color_label_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act & Assert
        assert!(!photo.has_color_label());

        photo.set_color_label(ColorLabel::Red);
        assert!(photo.has_color_label());
        assert_eq!(photo.color_label(), Some(ColorLabel::Red));

        photo.set_color_label(ColorLabel::Blue);
        assert_eq!(photo.color_label(), Some(ColorLabel::Blue));

        photo.remove_color_label();
        assert!(!photo.has_color_label());
    }

    #[test]
    fn test_edit_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act & Assert
        assert!(!photo.is_edited());

        photo.mark_as_edited();
        assert!(photo.is_edited());

        photo.reset_edits();
        assert!(!photo.is_edited());
    }

    #[test]
    fn test_combined_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act - Simular workflow real de usuário
        photo.rate(Rating::FOUR).unwrap();
        photo.set_color_label(ColorLabel::Green);
        photo.mark_as_edited();

        // Assert
        assert_eq!(photo.rating(), Some(Rating::FOUR));
        assert_eq!(photo.color_label(), Some(ColorLabel::Green));
        assert!(photo.is_edited());
        assert!(photo.has_rating());
        assert!(photo.has_color_label());
    }
}
