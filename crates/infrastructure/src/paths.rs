use directories::UserDirs;
use std::ffi::OsString;
use std::path::PathBuf;

/// Diz onde fica o catálogo, no lugar do caminho fixo em `~/Pictures`.
///
/// # Por que existe
///
/// Sem isto não há **teste automatizável contra catálogo descartável**: qualquer
/// teste que rodasse o app de verdade escreveria no catálogo real do fotógrafo,
/// e um teste que suja a biblioteca de fotos de alguém não é um teste que se
/// pode rodar. É pré-condição da migração para GPUI, onde os dois apps
/// (`ui` e `ui-gpui`) precisam abrir o mesmo catálogo — ou catálogos separados,
/// enquanto um está sendo construído.
///
/// Serve também ao caso real de quem guarda fotos num disco externo, e a quem o
/// SO não sabe dizer onde fica "Pictures" — antes disso, `catalog_root()`
/// entrava em pânico e o app não subia.
///
/// O prefixo `VLB_` é o que `VLB_SNAPSHOT` já usa nos testes de UI.
pub const VAR_DO_CATALOGO: &str = "VLB_CATALOG";

pub struct AppPaths;

impl AppPaths {
    /// Returns the main catalog root directory:
    /// - macOS: ~/Pictures/VintageLightbox/VintageLightbox Catalog
    /// - Windows: C:\Users\Use\Pictures\VintageLightbox\VintageLightbox Catalog
    /// - Linux: ~/Pictures/VintageLightbox/VintageLightbox Catalog
    ///
    /// [`VAR_DO_CATALOGO`] vence o padrão quando está definida e não está vazia.
    pub fn catalog_root() -> PathBuf {
        Self::catalog_root_de(std::env::var_os(VAR_DO_CATALOGO))
    }

    /// A decisão, separada da leitura do ambiente.
    ///
    /// Variável de ambiente é estado **global do processo**: um teste que a
    /// define muda o mundo dos que rodam em paralelo, e o resultado é o teste
    /// intermitente que parece defeito e não é. Recebendo o valor por
    /// parâmetro, a regra fica conferível sem que teste nenhum toque no
    /// ambiente.
    fn catalog_root_de(configurado: Option<OsString>) -> PathBuf {
        // Vazio é tratado como ausente de propósito: `VLB_CATALOG=` num script
        // de shell é o jeito comum de "desligar" a variável, e aceitá-lo como
        // caminho apontaria o catálogo para o diretório atual.
        if let Some(caminho) = configurado.filter(|v| !v.is_empty()) {
            return PathBuf::from(caminho);
        }

        let user_dirs = UserDirs::new().expect("Could not find user directories");
        let picture_dir = user_dirs
            .picture_dir()
            .expect("Could not find Pictures directory");

        picture_dir
            .join("VintageLightbox")
            .join("VintageLightbox Catalog")
    }

    /// Returns the path to the main SQLite database file
    pub fn main_db_path() -> PathBuf {
        Self::catalog_root().join("vintage_lightbox.db")
    }

    /// Returns the path to the Preview Cache directory (Previews.lrdata)
    pub fn preview_cache_dir() -> PathBuf {
        Self::catalog_root().join("Previews.lrdata")
    }

    /// Home do usuário logado, ou `/` se o SO não souber dizer
    pub fn home_dir() -> PathBuf {
        UserDirs::new()
            .map(|d| d.home_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/"))
    }

    /// Onde um seletor de pastas deve abrir por padrão
    ///
    /// Pictures do usuário, caindo para a home. Nunca a raiz do disco: abrir em `/` obriga
    /// o fotógrafo a navegar `Users` → nome → Pictures toda vez, e nenhuma das pastas de
    /// sistema listadas ali tem foto dele.
    pub fn default_browse_dir() -> PathBuf {
        UserDirs::new()
            .and_then(|d| d.picture_dir().map(|p| p.to_path_buf()))
            .filter(|p| p.exists())
            .unwrap_or_else(Self::home_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seletor_nunca_abre_na_raiz() {
        let inicial = AppPaths::default_browse_dir();

        assert_ne!(inicial, PathBuf::from("/"));
        assert!(inicial.exists(), "{:?} tem de existir", inicial);
    }

    #[test]
    fn a_variavel_manda_no_lugar_do_padrao() {
        let escolhido = AppPaths::catalog_root_de(Some(OsString::from("/tmp/catalogo-de-teste")));

        assert_eq!(escolhido, PathBuf::from("/tmp/catalogo-de-teste"));
    }

    #[test]
    fn sem_variavel_vale_o_caminho_de_sempre() {
        // O padrão não muda por causa desta entrega: quem já tem catálogo em
        // `~/Pictures` continua abrindo o mesmo.
        let padrao = AppPaths::catalog_root_de(None);

        assert!(padrao.ends_with("VintageLightbox/VintageLightbox Catalog"));
    }

    #[test]
    fn variavel_vazia_conta_como_ausente() {
        // `VLB_CATALOG=` num script é como se desliga a variável no shell.
        // Aceitá-la como caminho apontaria o catálogo para o diretório atual —
        // e criaria um catálogo novo onde quer que o app tenha sido iniciado.
        let vazia = AppPaths::catalog_root_de(Some(OsString::new()));

        assert_eq!(vazia, AppPaths::catalog_root_de(None));
    }

    #[test]
    fn o_banco_e_o_cache_seguem_o_catalogo() {
        // Os dois derivam de `catalog_root`, então apontar a variável leva o
        // catálogo inteiro junto — se um deles passasse a montar o caminho por
        // conta própria, o app abriria o banco de teste com o cache de produção.
        let raiz = AppPaths::catalog_root();

        assert!(AppPaths::main_db_path().starts_with(&raiz));
        assert!(AppPaths::preview_cache_dir().starts_with(&raiz));
    }
}
