use domain::entities::PresetId;
use domain::repositories::PresetRepository;
use domain::{DomainError, DomainResult};
use std::sync::Arc;

/// Troca o nome de uma predefinição do fotógrafo.
///
/// 🔑 **Renomear não é salvar de novo.** O caminho de salvar cria um `PresetId`
/// novo, e usá-lo aqui deixaria duas linhas no banco — a velha com o nome antigo
/// e a nova com o atual —, que na lista aparecem como duas predefinições iguais.
/// Este lê pelo id, troca o nome e regrava **a mesma linha**.
///
/// ⚠️ **Predefinição de sistema não se renomeia**, e a recusa é aqui e não na
/// tela: as sete nascem em código a cada listagem, então gravar uma com nome
/// novo criaria uma cópia no banco que a próxima abertura mostraria **ao lado**
/// da original, que voltaria com o nome de sempre.
pub struct RenamePresetUseCase {
    preset_repository: Arc<dyn PresetRepository>,
}

impl RenamePresetUseCase {
    pub fn new(preset_repository: Arc<dyn PresetRepository>) -> Self {
        Self { preset_repository }
    }

    pub async fn execute(&self, id: &PresetId, nome: String) -> DomainResult<()> {
        let nome = nome.trim().to_string();
        if nome.is_empty() {
            return Err(DomainError::InvalidOperation(
                "uma predefinição sem nome não dá para escolher na lista".into(),
            ));
        }

        let Some(mut preset) = self.preset_repository.find_by_id(id).await? else {
            return Err(DomainError::InvalidOperation(format!(
                "a predefinição {id} não existe"
            )));
        };
        if preset.is_system {
            return Err(DomainError::InvalidOperation(
                "as predefinições do sistema não se renomeiam".into(),
            ));
        }

        preset.name = nome;
        self.preset_repository.save(&preset).await
    }
}
