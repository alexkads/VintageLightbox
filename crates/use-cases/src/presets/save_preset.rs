use domain::entities::Preset;
use domain::repositories::PresetRepository;
use domain::DomainResult;
use std::sync::Arc;

/// Grava uma predefinição que **já tem id**.
///
/// 🚨 **Ela recebe o `Preset` pronto, e não nome e ajustes.** Criar a entidade
/// aqui dentro dava um `PresetId` que só a camada de dentro conhecia: a tela
/// punha na lista uma cópia com id **próprio**, e os dois lados passavam a
/// falar de linhas diferentes. Enquanto o único gesto era salvar, ninguém
/// notava; com renomear e apagar, o `UPDATE` e o `DELETE` iam para um id que a
/// tabela não tem — a predefinição sumia da tela e voltava na abertura
/// seguinte.
///
/// Quem cria a identidade é `Preset::user`, do lado de quem vai mostrá-la.
pub struct SavePresetUseCase {
    preset_repository: Arc<dyn PresetRepository>,
}

impl SavePresetUseCase {
    pub fn new(preset_repository: Arc<dyn PresetRepository>) -> Self {
        Self { preset_repository }
    }

    pub async fn execute(&self, preset: &Preset) -> DomainResult<()> {
        self.preset_repository.save(preset).await
    }
}
