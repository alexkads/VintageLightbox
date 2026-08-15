//! Repository Traits
//!
//! Define os contratos para persistência de dados seguindo Repository Pattern.
//! Implementados nas camadas de infraestrutura.

use crate::{
    entities::{Collection, Photo},
    value_objects::{CollectionId, PhotoId},
    DomainResult,
};
use async_trait::async_trait;

/// Repository para persistência de Photos
#[async_trait]
pub trait PhotoRepository: Send + Sync {
    /// Salva uma foto
    async fn save(&self, photo: &Photo) -> DomainResult<()>;

    /// Busca uma foto por ID
    async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>>;

    /// Lista todas as fotos
    async fn find_all(&self) -> DomainResult<Vec<Photo>>;

    /// Atualiza uma foto existente
    async fn update(&self, photo: &Photo) -> DomainResult<()>;

    /// Remove uma foto
    async fn delete(&self, id: &PhotoId) -> DomainResult<()>;

    /// Verifica se uma foto existe
    async fn exists(&self, id: &PhotoId) -> DomainResult<bool>;

    /// Busca uma foto pelo content hash (SHA-256)
    /// Retorna None se não encontrar nenhuma foto com esse hash
    async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>>;
}

/// Repository para persistência de Collections
#[async_trait]
pub trait CollectionRepository: Send + Sync {
    /// Salva uma coleção
    async fn save(&self, collection: &Collection) -> DomainResult<()>;

    /// Busca uma coleção por ID
    async fn find_by_id(&self, id: &CollectionId) -> DomainResult<Option<Collection>>;

    /// Lista todas as coleções
    async fn find_all(&self) -> DomainResult<Vec<Collection>>;

    /// Atualiza uma coleção existente
    async fn update(&self, collection: &Collection) -> DomainResult<()>;

    /// Remove uma coleção
    async fn delete(&self, id: &CollectionId) -> DomainResult<()>;

    /// Busca coleções que contêm uma foto específica
    async fn find_by_photo(&self, photo_id: &PhotoId) -> DomainResult<Vec<Collection>>;
}

// /// Trait para repositório de fotos
// pub trait PhotoRepository {
//     fn save(&self, photo: &Photo) -> DomainResult<()>;
//     fn find_by_id(&self, id: PhotoId) -> DomainResult<Option<Photo>>;
//     fn find_all(&self) -> DomainResult<Vec<Photo>>;
//     fn delete(&self, id: PhotoId) -> DomainResult<()>;
// }

use crate::entities::{Preset, PresetId};

/// Repository para persistência de Presets
#[async_trait]
pub trait PresetRepository: Send + Sync {
    /// Salva um preset
    async fn save(&self, preset: &Preset) -> DomainResult<()>;

    /// Busca um preset por ID
    async fn find_by_id(&self, id: &PresetId) -> DomainResult<Option<Preset>>;

    /// Lista todos os presets
    async fn find_all(&self) -> DomainResult<Vec<Preset>>;

    /// Remove um preset
    async fn delete(&self, id: &PresetId) -> DomainResult<()>;
}
