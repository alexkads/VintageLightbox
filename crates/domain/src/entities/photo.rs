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
use crate::value_objects::PhotoMetadata;

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
    /// Metadados técnicos (EXIF)
    metadata: Option<PhotoMetadata>,
    /// Caminho do thumbnail
    thumbnail_path: Option<FilePath>,
    /// Ajuste de exposição (persistence)
    edit_exposure: Option<f32>,
    /// Ajuste de contraste (persistence)
    edit_contrast: Option<f32>,
    /// Ajuste de temperatura (white balance warm/cool)
    edit_temperature: Option<f32>,
    /// Ajuste de tint (green/magenta)
    edit_tint: Option<f32>,
    /// Ajuste de highlights (bright areas)
    edit_highlights: Option<f32>,
    /// Ajuste de shadows (dark areas)
    edit_shadows: Option<f32>,
    /// Ajuste de whites (brightest whites)
    edit_whites: Option<f32>,
    /// Ajuste de blacks (darkest blacks)
    edit_blacks: Option<f32>,
    /// Ajuste de clarity (local contrast/sharpness)
    edit_clarity: Option<f32>,
    /// Ajuste de vibrance (intelligent saturation)
    edit_vibrance: Option<f32>,
    /// Ajuste de saturation (overall color intensity)
    edit_saturation: Option<f32>,
}

impl Photo {
    /// Cria uma nova foto gerando automaticamente um ID único
    pub fn new(file_path: FilePath) -> Self {
        Self::with_id(PhotoId::new(), file_path)
    }

    /// Reconstrói uma foto a partir de dados persistidos (uso interno/infraestrutura)
    pub fn reconstruct(
        id: PhotoId,
        file_path: FilePath,
        imported_at: DateTime<Utc>,
        modified_at: DateTime<Utc>,
        metadata: Option<PhotoMetadata>,
        rating: Option<Rating>,
        color_label: Option<ColorLabel>,
        is_edited: bool,
        thumbnail_path: Option<FilePath>,
        edit_exposure: Option<f32>,
        edit_contrast: Option<f32>,
        edit_temperature: Option<f32>,
        edit_tint: Option<f32>,
        edit_highlights: Option<f32>,
        edit_shadows: Option<f32>,
        edit_whites: Option<f32>,
        edit_blacks: Option<f32>,
        edit_clarity: Option<f32>,
        edit_vibrance: Option<f32>,
        edit_saturation: Option<f32>,
    ) -> Self {
        Self {
            id,
            file_path,
            imported_at,
            modified_at,
            metadata,
            rating,
            color_label,
            is_edited,
            thumbnail_path,
            edit_exposure,
            edit_contrast,
            edit_temperature,
            edit_tint,
            edit_highlights,
            edit_shadows,
            edit_whites,
            edit_blacks,
            edit_clarity,
            edit_vibrance,
            edit_saturation,
        }
    }

    /// Cria uma nova foto com ID específico (para testes/migrações)
    pub fn with_id(id: PhotoId, file_path: FilePath) -> Self {
        let now = Utc::now();
        Self::reconstruct(
            id, file_path, now, now, None, None, None, false, None,
            None, None, None, None, None, None, None, None, None, None, None
        )
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

    /// Retorna os metadados da foto
    pub fn metadata(&self) -> Option<&PhotoMetadata> {
        self.metadata.as_ref()
    }

    /// Define os metadados da foto
    pub fn set_metadata(&mut self, metadata: PhotoMetadata) {
        self.metadata = Some(metadata);
        self.modified_at = Utc::now();
    }

    /// Retorna o caminho do thumbnail
    pub fn thumbnail_path(&self) -> Option<&FilePath> {
        self.thumbnail_path.as_ref()
    }

    /// Define o caminho do thumbnail
    pub fn set_thumbnail_path(&mut self, path: FilePath) {
        self.thumbnail_path = Some(path);
        self.modified_at = Utc::now();
    }

    /// Retorna o campo edit_exposure
    pub fn edit_exposure(&self) -> Option<f32> {
        self.edit_exposure
    }

    /// Retorna o campo edit_contrast
    pub fn edit_contrast(&self) -> Option<f32> {
        self.edit_contrast
    }

    /// Retorna o campo edit_temperature
    pub fn edit_temperature(&self) -> Option<f32> {
        self.edit_temperature
    }

    /// Retorna o campo edit_tint
    pub fn edit_tint(&self) -> Option<f32> {
        self.edit_tint
    }

    /// Retorna o campo edit_highlights
    pub fn edit_highlights(&self) -> Option<f32> {
        self.edit_highlights
    }

    /// Retorna o campo edit_shadows
    pub fn edit_shadows(&self) -> Option<f32> {
        self.edit_shadows
    }

    /// Retorna o campo edit_whites
    pub fn edit_whites(&self) -> Option<f32> {
        self.edit_whites
    }

    /// Retorna o campo edit_blacks
    pub fn edit_blacks(&self) -> Option<f32> {
        self.edit_blacks
    }

    /// Retorna o campo edit_clarity
    pub fn edit_clarity(&self) -> Option<f32> {
        self.edit_clarity
    }

    /// Retorna o campo edit_vibrance
    pub fn edit_vibrance(&self) -> Option<f32> {
        self.edit_vibrance
    }

    /// Retorna o campo edit_saturation
    pub fn edit_saturation(&self) -> Option<f32> {
        self.edit_saturation
    }

    /// Define os ajustes de edição e marca como editada
    pub fn set_edits(
        &mut self,
        exposure: Option<f32>,
        contrast: Option<f32>,
        temperature: Option<f32>,
        tint: Option<f32>,
        highlights: Option<f32>,
        shadows: Option<f32>,
        whites: Option<f32>,
        blacks: Option<f32>,
        clarity: Option<f32>,
        vibrance: Option<f32>,
        saturation: Option<f32>,
    ) -> DomainResult<()> {
        self.edit_exposure = exposure;
        self.edit_contrast = contrast;
        self.edit_temperature = temperature;
        self.edit_tint = tint;
        self.edit_highlights = highlights;
        self.edit_shadows = shadows;
        self.edit_whites = whites;
        self.edit_blacks = blacks;
        self.edit_clarity = clarity;
        self.edit_vibrance = vibrance;
        self.edit_saturation = saturation;
        self.is_edited = true;
        self.modified_at = Utc::now();
        Ok(())
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
        let now = Utc::now();
        
        // Usar reconstruct para garantir timestamps idênticos
        // Usar reconstruct para garantir timestamps idênticos
        let photo1 = Photo::reconstruct(
            id, path.clone(), now, now, None, None, None, false, None,
            None, None, None, None, None, None, None, None, None, None, None
        );
        let photo2 = Photo::reconstruct(
            id, path, now, now, None, None, None, false, None,
            None, None, None, None, None, None, None, None, None, None, None
        );

        // Assert
        assert_eq!(photo1, photo2);
    }

    #[test]
    fn test_set_edits() {
        let file_path = FilePath::new("/photos/test.jpg").unwrap();
        let mut photo = Photo::new(file_path);

        assert!(photo.edit_exposure().is_none());
        assert!(photo.edit_contrast().is_none());
        assert!(!photo.is_edited());

        photo.set_edits(
            Some(1.5), Some(1.2), None, None, None, None, None, None, None, None, None
        ).unwrap();

        assert_eq!(photo.edit_exposure(), Some(1.5));
        assert_eq!(photo.edit_contrast(), Some(1.2));
        assert!(photo.is_edited());
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
