//! O controller das coleções — a camada que faltava.
//!
//! 🚨 **A entidade, o repositório e os três use cases existiam e estavam
//! testados; nunca houve controller.** Era o vão entre "pronto na camada de
//! dentro" e "alcançável por um clique", o mesmo que deixou a exportação
//! inalcançável por meses.
//!
//! 🔑 **E coleção não é uma funcionalidade a mais do Lightroom aqui**: o ensaio
//! de um cliente **é** uma coleção, e é dela que a galeria do
//! `recordarfotos.com.br` vai ser montada.

use std::sync::Arc;

use domain::entities::Collection;
use domain::repositories::CollectionRepository;
use domain::value_objects::{CollectionId, PhotoId};
use use_cases::{
    AddPhotoToCollectionUseCase, CreateCollectionUseCase, RemovePhotoFromCollectionUseCase,
};

/// Uma coleção, do jeito que a tela precisa dela.
///
/// ⚠️ **Leva a contagem, e não os ids.** Uma tela que mostra 40 coleções não tem
/// o que fazer com 40 conjuntos de ids, e carregá-los faria a lista lateral
/// custar o acervo inteiro em memória. Quem precisa dos ids é quem abriu uma.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionViewModel {
    pub id: String,
    pub name: String,
    pub photo_count: usize,
}

impl From<&Collection> for CollectionViewModel {
    fn from(colecao: &Collection) -> Self {
        Self {
            id: colecao.id().to_string(),
            name: colecao.name().to_string(),
            photo_count: colecao.photo_count(),
        }
    }
}

pub struct CollectionController {
    repositorio: Arc<dyn CollectionRepository>,
    criar: Arc<CreateCollectionUseCase>,
    adicionar: Arc<AddPhotoToCollectionUseCase>,
    remover: Arc<RemovePhotoFromCollectionUseCase>,
}

impl CollectionController {
    pub fn new(
        repositorio: Arc<dyn CollectionRepository>,
        criar: Arc<CreateCollectionUseCase>,
        adicionar: Arc<AddPhotoToCollectionUseCase>,
        remover: Arc<RemovePhotoFromCollectionUseCase>,
    ) -> Self {
        Self {
            repositorio,
            criar,
            adicionar,
            remover,
        }
    }

    pub async fn list(&self) -> Result<Vec<CollectionViewModel>, String> {
        let colecoes = self
            .repositorio
            .find_all()
            .await
            .map_err(|e| e.to_string())?;
        Ok(colecoes.iter().map(CollectionViewModel::from).collect())
    }

    /// Os ids das fotos de uma coleção — para a grade poder filtrar por ela.
    pub async fn photo_ids(&self, collection_id: String) -> Result<Vec<String>, String> {
        let id = CollectionId::from_string(&collection_id).map_err(|e| e.to_string())?;
        let colecao = self
            .repositorio
            .find_by_id(&id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "coleção não encontrada".to_string())?;

        Ok(colecao.photo_ids().iter().map(|p| p.to_string()).collect())
    }

    pub async fn create(&self, name: String) -> Result<CollectionViewModel, String> {
        let colecao = self
            .criar
            .execute(name, None)
            .await
            .map_err(|e| e.to_string())?;
        Ok(CollectionViewModel::from(&colecao))
    }

    /// Acrescenta várias fotos de uma vez.
    ///
    /// 🔑 **O lote é o caso normal, e não o caso raro.** Montar o ensaio de um
    /// cliente é selecionar 40 fotos e mandá-las para a coleção; uma chamada por
    /// foto seriam 40 idas ao banco, e — pior — 40 oportunidades de o lote parar
    /// pela metade sem que a tela soubesse em qual.
    ///
    /// ⚠️ **Uma foto que já está na coleção não é erro.** `Collection::add_photo`
    /// devolve `false` e segue; recusar o lote inteiro por causa dela faria
    /// "acrescentar as que faltam" ser impossível.
    pub async fn add_photos(
        &self,
        collection_id: String,
        photo_ids: Vec<String>,
    ) -> Result<usize, String> {
        let colecao = CollectionId::from_string(&collection_id).map_err(|e| e.to_string())?;

        let mut acrescentadas = 0usize;
        for id in photo_ids {
            let foto = PhotoId::from_string(&id).map_err(|e| e.to_string())?;
            self.adicionar
                .execute(colecao, foto)
                .await
                .map_err(|e| e.to_string())?;
            acrescentadas += 1;
        }
        Ok(acrescentadas)
    }

    pub async fn remove_photos(
        &self,
        collection_id: String,
        photo_ids: Vec<String>,
    ) -> Result<usize, String> {
        let colecao = CollectionId::from_string(&collection_id).map_err(|e| e.to_string())?;

        let mut removidas = 0usize;
        for id in photo_ids {
            let foto = PhotoId::from_string(&id).map_err(|e| e.to_string())?;
            // ⚠️ Remover o que não está lá **não** é erro para quem clicou: o
            // desfecho pedido ("esta foto não está mais na coleção") já vale.
            if self.remover.execute(colecao, foto).await.is_ok() {
                removidas += 1;
            }
        }
        Ok(removidas)
    }
}
