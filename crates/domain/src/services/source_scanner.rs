//! Source Scanner Service
//!
//! Trait para varrer uma origem (pasta ou cartão) listando as fotos que existem lá.
//!
//! É deliberadamente diferente de importar: a tela de importação precisa **mostrar** o que
//! existe na origem antes de qualquer arquivo ser tocado, e o usuário decide item a item o
//! que entra no catálogo.

use crate::{value_objects::FilePath, DomainResult};
use async_trait::async_trait;

/// Trait para varredura de origens de importação
#[async_trait]
pub trait SourceScanner: Send + Sync {
    /// Lista os arquivos de foto de uma origem, sem importar nada
    ///
    /// # Arguments
    /// * `root` - Pasta raiz da origem (ex: `/Volumes/CARTAO/DCIM`)
    /// * `include_subfolders` - Se desce em subpastas
    ///
    /// # Returns
    /// Caminhos das fotos encontradas, ordenados por caminho
    async fn scan(&self, root: &str, include_subfolders: bool) -> DomainResult<Vec<FilePath>>;
}
