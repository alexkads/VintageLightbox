use directories::UserDirs;
use std::path::PathBuf;

pub struct AppPaths;

impl AppPaths {
    /// Returns the main catalog root directory:
    /// - macOS: ~/Pictures/VintageLightbox/VintageLightbox Catalog
    /// - Windows: C:\Users\Use\Pictures\VintageLightbox\VintageLightbox Catalog
    /// - Linux: ~/Pictures/VintageLightbox/VintageLightbox Catalog
    pub fn catalog_root() -> PathBuf {
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
}
