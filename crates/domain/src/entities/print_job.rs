//! PrintJob Entity
//!
//! Representa um trabalho de impressão com fotos e configurações.
//! Implementado usando TDD (Test-Driven Development).

use crate::{
    value_objects::{PhotoId, PrintJobId, PrintLayout, PrintSettings},
    DomainError, DomainResult,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Entidade que representa um trabalho de impressão
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintJob {
    id: PrintJobId,
    photo_ids: Vec<PhotoId>,
    settings: PrintSettings,
    layout: PrintLayout,
    include_metadata: bool,
    color_profile: Option<String>,
    created_at: DateTime<Utc>,
}

impl PrintJob {
    /// Cria um novo trabalho de impressão
    pub fn new(
        photo_ids: Vec<PhotoId>,
        settings: PrintSettings,
        layout: PrintLayout,
    ) -> DomainResult<Self> {
        // Valida que há pelo menos uma foto
        if photo_ids.is_empty() {
            return Err(DomainError::InvalidOperation(
                "Print job must have at least one photo".to_string(),
            ));
        }

        Ok(PrintJob {
            id: PrintJobId::new(),
            photo_ids,
            settings,
            layout,
            include_metadata: false,
            color_profile: None,
            created_at: Utc::now(),
        })
    }

    /// Cria um trabalho de impressão com ID específico (para reconstrução do banco)
    pub fn with_id(
        id: PrintJobId,
        photo_ids: Vec<PhotoId>,
        settings: PrintSettings,
        layout: PrintLayout,
        include_metadata: bool,
        color_profile: Option<String>,
        created_at: DateTime<Utc>,
    ) -> DomainResult<Self> {
        if photo_ids.is_empty() {
            return Err(DomainError::InvalidOperation(
                "Print job must have at least one photo".to_string(),
            ));
        }

        Ok(PrintJob {
            id,
            photo_ids,
            settings,
            layout,
            include_metadata,
            color_profile,
            created_at,
        })
    }

    // Getters
    pub fn id(&self) -> PrintJobId {
        self.id
    }

    pub fn photo_ids(&self) -> &[PhotoId] {
        &self.photo_ids
    }

    pub fn settings(&self) -> &PrintSettings {
        &self.settings
    }

    pub fn layout(&self) -> PrintLayout {
        self.layout
    }

    pub fn include_metadata(&self) -> bool {
        self.include_metadata
    }

    pub fn color_profile(&self) -> Option<&str> {
        self.color_profile.as_deref()
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    // Setters
    pub fn set_settings(&mut self, settings: PrintSettings) {
        self.settings = settings;
    }

    pub fn set_layout(&mut self, layout: PrintLayout) {
        self.layout = layout;
    }

    pub fn set_include_metadata(&mut self, include: bool) {
        self.include_metadata = include;
    }

    pub fn set_color_profile(&mut self, profile: Option<String>) {
        self.color_profile = profile;
    }

    /// Adiciona uma foto ao trabalho
    pub fn add_photo(&mut self, photo_id: PhotoId) {
        if !self.photo_ids.contains(&photo_id) {
            self.photo_ids.push(photo_id);
        }
    }

    /// Remove uma foto do trabalho
    pub fn remove_photo(&mut self, photo_id: &PhotoId) -> DomainResult<()> {
        if self.photo_ids.len() == 1 {
            return Err(DomainError::InvalidOperation(
                "Cannot remove last photo from print job".to_string(),
            ));
        }

        self.photo_ids.retain(|id| id != photo_id);
        Ok(())
    }

    /// Retorna o número de fotos no trabalho
    pub fn photo_count(&self) -> usize {
        self.photo_ids.len()
    }

    /// Calcula o número de páginas necessárias
    pub fn page_count(&self) -> usize {
        let photos_per_page = self.layout.photos_per_page() as usize;
        (self.photo_count() + photos_per_page - 1) / photos_per_page
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value_objects::{PaperSize, Orientation, ColorMode, Margins};

    // 🔴 RED -> 🟢 GREEN -> 🔵 REFACTOR

    fn create_test_photo_ids(count: usize) -> Vec<PhotoId> {
        (0..count).map(|_| PhotoId::new()).collect()
    }

    fn create_test_settings() -> PrintSettings {
        PrintSettings::new(
            PaperSize::A4,
            Orientation::Portrait,
            Margins::default_margins(),
            300,
            ColorMode::Color,
        )
        .unwrap()
    }

    #[test]
    fn test_print_job_creation_with_valid_photos() {
        let photo_ids = create_test_photo_ids(3);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let job = PrintJob::new(photo_ids.clone(), settings, layout);
        assert!(job.is_ok());

        let job = job.unwrap();
        assert_eq!(job.photo_ids().len(), 3);
        assert_eq!(job.photo_count(), 3);
        assert!(!job.include_metadata());
        assert!(job.color_profile().is_none());
    }

    #[test]
    fn test_print_job_creation_with_empty_photos() {
        let photo_ids = vec![];
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let job = PrintJob::new(photo_ids, settings, layout);
        assert!(job.is_err());
    }

    #[test]
    fn test_print_job_add_photo() {
        let photo_ids = create_test_photo_ids(2);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids, settings, layout).unwrap();
        assert_eq!(job.photo_count(), 2);

        let new_photo = PhotoId::new();
        job.add_photo(new_photo);
        assert_eq!(job.photo_count(), 3);
    }

    #[test]
    fn test_print_job_add_duplicate_photo() {
        let photo_ids = create_test_photo_ids(2);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids.clone(), settings, layout).unwrap();
        assert_eq!(job.photo_count(), 2);

        // Tentar adicionar foto que já existe
        job.add_photo(photo_ids[0]);
        assert_eq!(job.photo_count(), 2); // Não deve duplicar
    }

    #[test]
    fn test_print_job_remove_photo() {
        let photo_ids = create_test_photo_ids(3);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids.clone(), settings, layout).unwrap();
        assert_eq!(job.photo_count(), 3);

        let result = job.remove_photo(&photo_ids[0]);
        assert!(result.is_ok());
        assert_eq!(job.photo_count(), 2);
    }

    #[test]
    fn test_print_job_remove_last_photo() {
        let photo_ids = create_test_photo_ids(1);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids.clone(), settings, layout).unwrap();
        assert_eq!(job.photo_count(), 1);

        let result = job.remove_photo(&photo_ids[0]);
        assert!(result.is_err()); // Não pode remover última foto
    }

    #[test]
    fn test_print_job_set_settings() {
        let photo_ids = create_test_photo_ids(2);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids, settings, layout).unwrap();

        let new_settings = PrintSettings::new(
            PaperSize::Letter,
            Orientation::Landscape,
            Margins::default_margins(),
            600,
            ColorMode::Grayscale,
        )
        .unwrap();

        job.set_settings(new_settings.clone());
        assert_eq!(job.settings(), &new_settings);
    }

    #[test]
    fn test_print_job_set_layout() {
        let photo_ids = create_test_photo_ids(4);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids, settings, layout).unwrap();

        let new_layout = PrintLayout::multiple(2, 2).unwrap();
        job.set_layout(new_layout);
        assert_eq!(job.layout(), new_layout);
    }

    #[test]
    fn test_print_job_set_metadata() {
        let photo_ids = create_test_photo_ids(2);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids, settings, layout).unwrap();
        assert!(!job.include_metadata());

        job.set_include_metadata(true);
        assert!(job.include_metadata());
    }

    #[test]
    fn test_print_job_set_color_profile() {
        let photo_ids = create_test_photo_ids(2);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let mut job = PrintJob::new(photo_ids, settings, layout).unwrap();
        assert!(job.color_profile().is_none());

        job.set_color_profile(Some("sRGB".to_string()));
        assert_eq!(job.color_profile(), Some("sRGB"));

        job.set_color_profile(None);
        assert!(job.color_profile().is_none());
    }

    #[test]
    fn test_print_job_page_count_single_layout() {
        let photo_ids = create_test_photo_ids(5);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;

        let job = PrintJob::new(photo_ids, settings, layout).unwrap();
        assert_eq!(job.page_count(), 5); // 5 fotos, 1 por página = 5 páginas
    }

    #[test]
    fn test_print_job_page_count_multiple_layout() {
        let photo_ids = create_test_photo_ids(10);
        let settings = create_test_settings();
        let layout = PrintLayout::multiple(2, 2).unwrap(); // 4 fotos por página

        let job = PrintJob::new(photo_ids, settings, layout).unwrap();
        assert_eq!(job.page_count(), 3); // 10 fotos, 4 por página = 3 páginas
    }

    #[test]
    fn test_print_job_page_count_contact_sheet() {
        let photo_ids = create_test_photo_ids(25);
        let settings = create_test_settings();
        let layout = PrintLayout::contact_sheet(12).unwrap(); // 12 fotos por página

        let job = PrintJob::new(photo_ids, settings, layout).unwrap();
        assert_eq!(job.page_count(), 3); // 25 fotos, 12 por página = 3 páginas
    }

    #[test]
    fn test_print_job_with_id() {
        let id = PrintJobId::new();
        let photo_ids = create_test_photo_ids(2);
        let settings = create_test_settings();
        let layout = PrintLayout::Single;
        let created_at = Utc::now();

        let job = PrintJob::with_id(
            id,
            photo_ids,
            settings,
            layout,
            true,
            Some("AdobeRGB".to_string()),
            created_at,
        );

        assert!(job.is_ok());
        let job = job.unwrap();
        assert_eq!(job.id(), id);
        assert!(job.include_metadata());
        assert_eq!(job.color_profile(), Some("AdobeRGB"));
        assert_eq!(job.created_at(), created_at);
    }
}
