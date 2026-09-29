//! Repositórios e exportador **em memória**, só para os testes dos controllers.
//!
//! 🔑 O `TODO` do `photo_controller` dizia que faltavam testes por causa de
//! "lifetimes do mockall". Um falso simples resolve sem o mockall e ainda deixa
//! o teste afirmar **o estado final** (a foto ficou com 4 estrelas), e não só
//! que um método foi chamado.

use async_trait::async_trait;
use domain::entities::{Collection, Photo};
use domain::repositories::{CollectionRepository, PhotoRepository};
use domain::services::ImageExporter;
use domain::value_objects::{CollectionId, ExportOptions, FilePath, PhotoId};
use domain::{DomainError, DomainResult};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
pub struct FotosEmMemoria {
    fotos: Mutex<HashMap<PhotoId, Photo>>,
}

impl FotosEmMemoria {
    pub fn com(fotos: impl IntoIterator<Item = Photo>) -> Self {
        let repo = Self::default();
        for f in fotos {
            repo.fotos.lock().unwrap().insert(f.id(), f);
        }
        repo
    }

    pub fn ler(&self, id: &PhotoId) -> Option<Photo> {
        self.fotos.lock().unwrap().get(id).cloned()
    }
}

#[async_trait]
impl PhotoRepository for FotosEmMemoria {
    async fn save(&self, photo: &Photo) -> DomainResult<()> {
        self.fotos.lock().unwrap().insert(photo.id(), photo.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>> {
        Ok(self.ler(id))
    }
    async fn find_all(&self) -> DomainResult<Vec<Photo>> {
        Ok(self.fotos.lock().unwrap().values().cloned().collect())
    }
    async fn update(&self, photo: &Photo) -> DomainResult<()> {
        self.save(photo).await
    }
    async fn delete(&self, id: &PhotoId) -> DomainResult<()> {
        self.fotos.lock().unwrap().remove(id);
        Ok(())
    }
    async fn exists(&self, id: &PhotoId) -> DomainResult<bool> {
        Ok(self.ler(id).is_some())
    }
    async fn find_by_content_hash(&self, _hash: &str) -> DomainResult<Option<Photo>> {
        Ok(None)
    }
}

#[derive(Default)]
pub struct ColecoesEmMemoria {
    colecoes: Mutex<HashMap<CollectionId, Collection>>,
}

impl ColecoesEmMemoria {
    pub fn com(colecoes: impl IntoIterator<Item = Collection>) -> Self {
        let repo = Self::default();
        for c in colecoes {
            repo.colecoes.lock().unwrap().insert(*c.id(), c);
        }
        repo
    }

    pub fn ler(&self, id: &CollectionId) -> Option<Collection> {
        self.colecoes.lock().unwrap().get(id).cloned()
    }
}

#[async_trait]
impl CollectionRepository for ColecoesEmMemoria {
    async fn save(&self, c: &Collection) -> DomainResult<()> {
        self.colecoes.lock().unwrap().insert(*c.id(), c.clone());
        Ok(())
    }
    async fn find_by_id(&self, id: &CollectionId) -> DomainResult<Option<Collection>> {
        Ok(self.ler(id))
    }
    async fn find_all(&self) -> DomainResult<Vec<Collection>> {
        let mut todas: Vec<_> = self.colecoes.lock().unwrap().values().cloned().collect();
        todas.sort_by(|a, b| a.name().cmp(b.name()));
        Ok(todas)
    }
    async fn update(&self, c: &Collection) -> DomainResult<()> {
        self.save(c).await
    }
    async fn delete(&self, id: &CollectionId) -> DomainResult<()> {
        self.colecoes.lock().unwrap().remove(id);
        Ok(())
    }
    async fn find_by_photo(&self, photo_id: &PhotoId) -> DomainResult<Vec<Collection>> {
        Ok(self
            .colecoes
            .lock()
            .unwrap()
            .values()
            .filter(|c| c.contains_photo(photo_id))
            .cloned()
            .collect())
    }
}

/// Registra o que foi pedido; não escreve arquivo nenhum.
#[derive(Default)]
pub struct ExportadorQueAnota {
    pub pedidos: Mutex<Vec<(PhotoId, String, u8)>>,
    pub falha: bool,
}

#[async_trait]
impl ImageExporter for ExportadorQueAnota {
    async fn export(
        &self,
        photo: &Photo,
        output_path: &FilePath,
        options: &ExportOptions,
    ) -> DomainResult<()> {
        if self.falha {
            return Err(DomainError::InfrastructureError("disco cheio".into()));
        }
        self.pedidos
            .lock()
            .unwrap()
            .push((photo.id(), output_path.to_string(), options.quality()));
        Ok(())
    }

    async fn renderizar_jpeg(
        &self,
        _photo: &Photo,
        _options: &ExportOptions,
    ) -> DomainResult<Vec<u8>> {
        Ok(vec![0xFF, 0xD8])
    }

    async fn renderizar_bruto_jpeg(
        &self,
        _photo: &Photo,
        _options: &ExportOptions,
    ) -> DomainResult<Option<Vec<u8>>> {
        Ok(None)
    }
}
