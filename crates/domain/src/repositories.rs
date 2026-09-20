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

/// Onde mora a revelação de uma foto que **só existe no site**, até ela subir.
///
/// # Por que ela não vai para `photos`
///
/// A foto aberta a partir de uma sessão do pós-venda tem o id `site:<uuid>` e
/// não é linha do catálogo: ela vive no storage da nuvem, e esta máquina nunca
/// a importou. Até 8/set/2026 isso significava que revelá-la não gravava nada —
/// `SavePhotoEditsUseCase` respondia `PhotoNotFound` a cada gesto, e a tela não
/// tinha como saber (o `Gravador` não devolve `Result`, de propósito).
///
/// # O que ela é, e o que ela não é
///
/// 🔑 **É o depósito do trabalho ainda não enviado** — o equivalente ao que o
/// site guarda no navegador (`lib/biblioteca/local.ts`). Os `ajustes` são
/// **opacos** aqui: o JSON que sobe para a API e volta dela, com os 53 por nome
/// e o enquadramento com prefixo `corte_`. Quem conhece os nomes é o motor.
///
/// 🚨 **Não é catálogo**: nada aqui faz a foto aparecer na Biblioteca. E **não é
/// cache**: ninguém regenera o que está guardado — é trabalho do operador.
///
/// A linha sai quando a revelação sobe ([`Self::esquecer`]): a partir daí a
/// verdade é o servidor, e guardar as duas abriria a pergunta de qual vale.
#[async_trait]
pub trait RevelacoesDoSiteRepository: Send + Sync {
    /// Guarda (ou substitui) a receita desta foto do site.
    async fn guardar(&self, foto_no_site: &str, ajustes: &str) -> DomainResult<()>;

    /// Todas as que ainda não subiram, como `(id no site, ajustes em JSON)`.
    ///
    /// Lidas de uma vez na abertura do app, como os presets: são poucas — o que
    /// está aqui é o que o operador revelou e ainda não salvou na galeria.
    async fn todas(&self) -> DomainResult<Vec<(String, String)>>;

    /// Tira esta foto do depósito — ela subiu, e o servidor passou a ser a
    /// verdade.
    async fn esquecer(&self, foto_no_site: &str) -> DomainResult<()>;
}
