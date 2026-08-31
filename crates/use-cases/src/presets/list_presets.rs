use domain::entities::preset::PresetAdjustments;
use domain::entities::Preset;
use domain::repositories::PresetRepository;
use domain::DomainResult;
use std::sync::Arc;

/// Os presets que existem antes de alguém salvar o primeiro.
///
/// Eles não moram na tabela `presets`: são construídos aqui a cada listagem, e a
/// tela os separa dos do usuário por `is_system`.
///
/// ## 🚨 Os valores estavam na escala errada — até 30/ago/2026
///
/// Os quatro pediam números de uma escala que o motor de revelação não usa, e o
/// resultado não era "pouco efeito", era foto destruída:
///
/// | Preset | Pedia | O que o motor faz com isso |
/// |---|--:|---|
/// | B&W | `saturation: -100` | o fator é `1 + s`, então `-99`: cada canal jogado 99× para o **lado oposto** do cinza — cor invertida e estourada, não ausência de cor |
/// | High Contrast | `contrast: 50` | é multiplicador em volta de 128, não porcentagem: `(v-128)*50+128` deixa a foto em preto e branco puro, sem meio-tom |
/// | Warm / Cool | `temperature: ±15` | o shader faz `r += t*10` em 0..255: são ±150 níveis, e a foto sai vermelha ou azul chapada |
///
/// **As escalas são as do shader** (`image_adjustments.wgsl`), e são as mesmas
/// faixas que os sliders da Revelação oferecem (`CONTROLES`, em
/// `ui-gpui/src/revelacao/controles.rs`): saturação -1..1 (cinza é o fator zero,
/// ou seja `-1`), contraste 0..2 com neutro em **1**, temperatura -10..10.
/// Preset que sai da faixa pede o que nenhum arrasto de slider consegue pedir —
/// e é assim que se reconhece o defeito.
///
/// ⚠️ **"Auto" saiu da lista em 30/ago/2026.** Ele pedia `exposure: Some(0.0)`
/// com um `// Placeholder` ao lado: clicar não fazia nada, que é a pior das três
/// situações (`docs/PARIDADE-LIGHTROOM.md`). E o lugar dele nunca foi este — no
/// Lightroom "Auto" é um **botão do painel Básico**, que lê a foto e escolhe os
/// tons a partir dela. Um preset é uma lista de números fixos, e nenhuma lista
/// fixa serve para todas as fotos.
pub fn presets_de_sistema() -> Vec<Preset> {
    vec![
        // Cinza é o fator zero, e o fator é `1 + saturation`.
        //
        // 🔑 Só a saturação basta: a intensidade (`vibrance`) vem **depois** dela
        // no shader e trabalha sobre `canal - luminância`, que num pixel cinza é
        // zero. O que já estava intenso na foto não recolore o preto e branco —
        // por isso o preset não precisa zerar a intensidade junto, e não zera.
        Preset::system(
            "B&W",
            PresetAdjustments {
                saturation: Some(-1.0),
                ..Default::default()
            },
        ),
        // `r += t*10`, `b -= t*10`, em 0..255: 1,5 são 15 níveis para cada lado.
        // É o número que o preset antigo escrevia — na unidade que ele queria
        // dizer.
        Preset::system(
            "Warm",
            PresetAdjustments {
                temperature: Some(1.5),
                ..Default::default()
            },
        ),
        Preset::system(
            "Cool",
            PresetAdjustments {
                temperature: Some(-1.5),
                ..Default::default()
            },
        ),
        // Multiplicador em volta de 128, com neutro em 1,0 e teto em 2,0.
        Preset::system(
            "High Contrast",
            PresetAdjustments {
                contrast: Some(1.35),
                ..Default::default()
            },
        ),
    ]
}

pub struct ListPresetsUseCase {
    preset_repository: Arc<dyn PresetRepository>,
}

impl ListPresetsUseCase {
    pub fn new(preset_repository: Arc<dyn PresetRepository>) -> Self {
        Self { preset_repository }
    }

    /// Os de sistema primeiro, na ordem em que foram escritos; os do usuário
    /// depois, na ordem do repositório.
    pub async fn execute(&self) -> DomainResult<Vec<Preset>> {
        let mut user_presets = self.preset_repository.find_all().await?;

        let mut all_presets = presets_de_sistema();
        all_presets.append(&mut user_presets);

        Ok(all_presets)
    }
}
