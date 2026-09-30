//! A revelação feita fora deste app que acompanha o RAW importado — o XMP do
//! Lightroom, dentro do DNG ou ao lado do NEF.

use std::path::Path;

use crate::entities::Photo;

/// Quem lê essa revelação e a grava na foto que está entrando.
///
/// Implementada em `infrastructure::revelacao_do_arquivo`; a importação não
/// sabe o que é XMP.
pub trait LeitorDaRevelacaoDoArquivo: Send + Sync {
    /// Grava em `foto` a revelação que acompanha o arquivo em `origem`.
    ///
    /// `None` quando não havia revelação; `Some(ignorados)` quando ela entrou,
    /// com o que o Lightroom fez e este motor não tem (vazio: entrou tudo).
    fn aplicar(&self, origem: &Path, foto: &mut Photo) -> Option<Vec<String>>;

    /// Leva o `.xmp` ao lado de `origem` para junto de `destino`, com o nome
    /// dele — a importação renomeia o RAW, e o `.xmp` sem o mesmo nome deixaria
    /// de ser dele. `mover` apaga o de origem, como a importação que move.
    fn levar_xmp_junto(&self, origem: &Path, destino: &Path, mover: bool);
}
