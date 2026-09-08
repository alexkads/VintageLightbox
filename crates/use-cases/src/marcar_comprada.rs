//! Marcar a foto que o cliente levou no balcão.
//!
//! É a decisão que o pós-venda do `recordarfotos.com.br` consome: levada vira
//! download liberado, deixada para trás vira foto à venda com marca d'água. Até
//! hoje ela não era gravada — era só qual botão se apertava na exportação.

use domain::{
    entities::Photo, repositories::PhotoRepository, value_objects::PhotoId, DomainError,
    DomainResult,
};
use std::sync::Arc;

pub struct MarcarCompradaUseCase {
    repo: Arc<dyn PhotoRepository>,
}

impl MarcarCompradaUseCase {
    pub fn new(repo: Arc<dyn PhotoRepository>) -> Self {
        Self { repo }
    }

    /// `true` marca, `false` desmarca. Idempotente: marcar o que já está
    /// marcado não reescreve a data (ver `Photo::marcar_comprada`).
    pub async fn execute(&self, id: PhotoId, comprada: bool) -> DomainResult<Photo> {
        let mut photo = self
            .repo
            .find_by_id(&id)
            .await?
            .ok_or(DomainError::PhotoNotFound)?;

        if comprada {
            photo.marcar_comprada();
        } else {
            photo.desmarcar_comprada();
        }

        self.repo.update(&photo).await?;
        Ok(photo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::value_objects::FilePath;
    use mockall::mock;
    use mockall::predicate::*;

    mock! {
        pub PhotoRepo {}

        #[async_trait::async_trait]
        impl PhotoRepository for PhotoRepo {
            async fn save(&self, photo: &Photo) -> DomainResult<()>;
            async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>>;
            async fn find_all(&self) -> DomainResult<Vec<Photo>>;
            async fn update(&self, photo: &Photo) -> DomainResult<()>;
            async fn delete(&self, id: &PhotoId) -> DomainResult<()>;
            async fn exists(&self, id: &PhotoId) -> DomainResult<bool>;
            async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>>;
        }
    }

    #[tokio::test]
    async fn marca_e_grava_a_foto_como_comprada() {
        let photo = Photo::new(FilePath::new("/photos/test.jpg").unwrap());
        let id = photo.id();

        let mut repo = MockPhotoRepo::new();
        let devolvida = photo.clone();
        repo.expect_find_by_id()
            .with(eq(id))
            .times(1)
            .returning(move |_| Ok(Some(devolvida.clone())));
        repo.expect_update()
            .withf(|p| p.comprada())
            .times(1)
            .returning(|_| Ok(()));

        let resultado = MarcarCompradaUseCase::new(Arc::new(repo))
            .execute(id, true)
            .await
            .unwrap();
        assert!(resultado.comprada());
    }

    #[tokio::test]
    async fn foto_inexistente_e_erro_e_nao_grava() {
        let mut repo = MockPhotoRepo::new();
        repo.expect_find_by_id().returning(|_| Ok(None));
        // Nenhuma expectativa de `update`: gravar seria falha.

        let erro = MarcarCompradaUseCase::new(Arc::new(repo))
            .execute(PhotoId::new(), true)
            .await
            .unwrap_err();
        assert!(matches!(erro, DomainError::PhotoNotFound));
    }
}
