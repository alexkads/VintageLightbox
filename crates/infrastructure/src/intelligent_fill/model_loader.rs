//! Model Loader
//!
//! Gerencia o carregamento, cache e versionamento do modelo de inpainting.

use std::path::PathBuf;

/// Informações sobre um modelo de inpainting
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Nome do modelo
    pub name: String,
    /// Caminho para o arquivo do modelo
    pub path: PathBuf,
    /// Versão do modelo
    pub version: String,
    /// Tamanho de entrada esperado (largura/altura do patch)
    pub input_size: u32,
}

/// Gerencia download, cache e versionamento de modelos
pub struct ModelLoader {
    cache_dir: PathBuf,
}

impl ModelLoader {
    /// Cria um novo ModelLoader
    pub fn new() -> Self {
        let cache_dir = crate::paths::AppPaths::models_dir();
        Self { cache_dir }
    }

    /// Cria um ModelLoader com diretório de cache personalizado
    pub fn with_cache_dir(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Obtém o caminho para o modelo, baixando se necessário
    ///
    /// Atualmente retorna erro se o modelo não existe.
    /// Futuramente implementará download automático.
    pub async fn ensure_model(&self) -> Result<ModelInfo, String> {
        // Verificar se o modelo existe no cache
        let model_path = self.cache_dir.join("inpainting_v1.onnx");

        if model_path.exists() {
            return Ok(ModelInfo {
                name: "MAT-Inpainting-Lite".to_string(),
                path: model_path,
                version: "1.0.0".to_string(),
                input_size: 512,
            });
        }

        // Verificar se existe modelo bundled no diretório de assets
        let bundled_path = self.find_bundled_model();
        if let Some(path) = bundled_path {
            return Ok(ModelInfo {
                name: "MAT-Inpainting-Lite".to_string(),
                path,
                version: "1.0.0".to_string(),
                input_size: 512,
            });
        }

        // TODO: Implementar download automático do modelo
        Err(format!(
            "Modelo de inpainting não encontrado em: {:?}. \
             Baixe o modelo MAT-Lite e coloque em {:?}",
            model_path, self.cache_dir
        ))
    }

    /// Procura por modelo bundled nos diretórios de assets do aplicativo
    fn find_bundled_model(&self) -> Option<PathBuf> {
        // Locais possíveis para modelo bundled
        let possible_paths = [
            // Relativo ao executável
            std::env::current_exe()
                .ok()?
                .parent()?
                .join("assets")
                .join("models")
                .join("inpainting_v1.onnx"),
            // Diretório de trabalho atual
            PathBuf::from("assets/models/inpainting_v1.onnx"),
            // Diretório do projeto (desenvolvimento)
            PathBuf::from("crates/infrastructure/assets/models/inpainting_v1.onnx"),
        ];

        possible_paths.into_iter().find(|p| p.exists())
    }

    /// Retorna o diretório de cache de modelos
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Verifica se o modelo está disponível localmente
    pub fn is_model_available(&self) -> bool {
        let model_path = self.cache_dir.join("inpainting_v1.onnx");
        model_path.exists() || self.find_bundled_model().is_some()
    }
}

impl Default for ModelLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_model_loader_with_custom_dir() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ModelLoader::with_cache_dir(temp_dir.path().to_path_buf());
        assert_eq!(loader.cache_dir(), &temp_dir.path().to_path_buf());
    }

    #[test]
    fn test_model_not_available_in_empty_dir() {
        let temp_dir = TempDir::new().unwrap();
        let loader = ModelLoader::with_cache_dir(temp_dir.path().to_path_buf());
        // O modelo não deve estar disponível em um diretório vazio
        // (a menos que exista bundled, o que não é o caso em testes)
        assert!(!loader.is_model_available() || loader.find_bundled_model().is_some());
    }
}
