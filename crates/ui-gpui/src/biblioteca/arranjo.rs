//! O arranjo dos painéis, gravado em disco.
//!
//! Arrumar a tela e perder a arrumação ao fechar o app é o mesmo que não poder
//! arrumar — e o legado grava o `DockState` das duas telas
//! (`eframe::set_value`), então isto é paridade, não enfeite.
//!
//! ## ⚠️ Ler daqui **nunca** derruba o app
//!
//! Toda falha vira `None`, e `None` é o arranjo padrão. Arquivo corrompido,
//! escrito por uma versão que não existe mais, ou de um dia em que os painéis
//! tinham outros nomes — em todos esses casos o certo é abrir com o padrão, e
//! não recusar-se a abrir. É o oposto da leitura do catálogo, onde falhar alto é
//! o que impede escrever em cima do dado de alguém.

use std::path::{Path, PathBuf};

use gpui_component::dock::DockAreaState;
use infrastructure::paths::AppPaths;

/// A versão do arranjo.
///
/// 🔑 **Ela existe para poder invalidar o que está gravado.** No dia em que um
/// painel for dividido em dois, ou a Biblioteca ganhar um quinto, o arranjo
/// salvo descreve uma tela que não existe mais — e restaurá-lo daria uma
/// Biblioteca sem grade, ou com um painel vazio no lugar dela. Subir este número
/// faz todo mundo voltar ao padrão de uma vez, que é o desfecho certo.
pub const VERSAO: usize = 1;

/// Onde o arranjo mora: ao lado do catálogo.
///
/// 🔑 **No catálogo, e não numa pasta de configuração do sistema.** O
/// `VLB_CATALOG` é o que separa o catálogo real do de medição — e com o arranjo
/// junto, rodar o app contra um catálogo descartável não mexe na arrumação da
/// tela de quem trabalha.
pub fn caminho() -> PathBuf {
    AppPaths::catalog_root().join("arranjo-biblioteca.json")
}

/// Lê o arranjo. Qualquer problema devolve `None`, que é "use o padrão".
pub fn ler_de(caminho: &Path) -> Option<DockAreaState> {
    let texto = std::fs::read_to_string(caminho).ok()?;
    let estado: DockAreaState = serde_json::from_str(&texto).ok()?;

    // ⚠️ **Versão diferente é arranjo de outra tela.** Restaurá-lo daria painéis
    // faltando ou sobrando, e quem abrisse veria a Biblioteca torta sem ter
    // mexido em nada.
    if estado.version != Some(VERSAO) {
        return None;
    }

    Some(estado)
}

/// Grava o arranjo. Falhar aqui **não interrompe nada**: o pior desfecho é a
/// arrumação não sobreviver ao fechamento, e derrubar o app por causa disso
/// seria trocar um aborrecimento por uma perda.
pub fn gravar_em(caminho: &Path, estado: &DockAreaState) {
    let Ok(texto) = serde_json::to_string_pretty(estado) else {
        return;
    };

    if let Some(pasta) = caminho.parent() {
        let _ = std::fs::create_dir_all(pasta);
    }
    if let Err(erro) = std::fs::write(caminho, texto) {
        eprintln!("⚠️  Não foi possível gravar o arranjo dos painéis: {erro}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    use tempfile::TempDir;

    fn estado_de_teste(versao: Option<usize>) -> DockAreaState {
        DockAreaState {
            version: versao,
            ..Default::default()
        }
    }

    #[test]
    fn o_arranjo_gravado_volta_igual() {
        let dir = TempDir::new().expect("diretório temporário");
        let arquivo = dir.path().join("arranjo.json");

        let estado = estado_de_teste(Some(VERSAO));
        gravar_em(&arquivo, &estado);

        assert_eq!(ler_de(&arquivo), Some(estado));
    }

    /// 🚨 Arranjo de outra versão é ignorado, e não restaurado torto.
    ///
    /// O número existe para o dia em que um painel for dividido ou nascer outro:
    /// o arranjo salvo descreve uma tela que não existe mais, e restaurá-lo daria
    /// uma Biblioteca sem grade — sem ninguém ter mexido em nada.
    #[test]
    fn arranjo_de_outra_versao_e_ignorado() {
        let dir = TempDir::new().expect("diretório temporário");
        let arquivo = dir.path().join("arranjo.json");

        gravar_em(&arquivo, &estado_de_teste(Some(VERSAO + 1)));
        assert_eq!(ler_de(&arquivo), None);

        gravar_em(&arquivo, &estado_de_teste(None));
        assert_eq!(ler_de(&arquivo), None, "sem versão também não vale");
    }

    /// 🚨 Arquivo corrompido abre o app no padrão — não o impede de abrir.
    ///
    /// É o oposto da leitura do catálogo, onde falhar alto é o que impede
    /// escrever em cima do dado de alguém. Aqui o pior que um arquivo ruim pode
    /// custar é a arrumação da tela.
    #[test]
    fn arquivo_corrompido_nao_impede_de_abrir() {
        let dir = TempDir::new().expect("diretório temporário");
        let arquivo = dir.path().join("arranjo.json");
        std::fs::write(&arquivo, "{ isto não é json").expect("escrever lixo");

        assert_eq!(ler_de(&arquivo), None);
    }

    /// E arquivo que não existe é o primeiro uso: também é `None`.
    #[test]
    fn sem_arquivo_e_o_primeiro_uso() {
        let dir = TempDir::new().expect("diretório temporário");

        assert_eq!(ler_de(&dir.path().join("nao-existe.json")), None);
    }

    /// Gravar numa pasta que não existe cria a pasta.
    #[test]
    fn gravar_cria_a_pasta_que_faltar() {
        let dir = TempDir::new().expect("diretório temporário");
        let arquivo = dir.path().join("uma/pasta/nova/arranjo.json");

        gravar_em(&arquivo, &estado_de_teste(Some(VERSAO)));

        assert!(arquivo.exists());
    }
}
