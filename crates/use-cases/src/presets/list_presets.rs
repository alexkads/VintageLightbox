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
/// # 🔑 São os sete do site, e são os mesmos números
///
/// Até 7/set/2026 eram quatro — "B&W", "Warm", "Cool" e "High Contrast" —, e o
/// site tinha outros sete, em português, guardados em
/// `revelacao/presets-do-sistema.ts`. Duas listas para o mesmo motor: quem
/// revelasse a mesma foto nos dois lugares não tinha um ponto de partida em
/// comum, e "aplique a Sépia" queria dizer coisas diferentes conforme a tela.
///
/// Agora são estes, campo a campo iguais aos de lá. **As escalas são as do
/// shader**, e não as do Lightroom: `contrast` é multiplicador com neutro 1,
/// `saturation` e `clarity` vão de −1 a 1, `temperature` de −10 a 10, e o resto
/// dos básicos de −100 a 100.
///
/// ## 🚨 Os valores estavam na escala errada — até 30/ago/2026
///
/// Os quatro antigos pediam números de uma escala que o motor de revelação não
/// usa, e o resultado não era "pouco efeito", era foto destruída:
///
/// | Preset | Pedia | O que o motor fazia com isso |
/// |---|--:|---|
/// | B&W | `saturation: -100` | o fator é `1 + s`, então `-99`: cada canal jogado 99× para o **lado oposto** do cinza — cor invertida e estourada, não ausência de cor |
/// | High Contrast | `contrast: 50` | é multiplicador em volta de 128, não porcentagem: `(v-128)*50+128` deixa a foto em preto e branco puro, sem meio-tom |
/// | Warm / Cool | `temperature: ±15` | o shader faz `r += t*10` em 0..255: são ±150 níveis, e a foto sai vermelha ou azul chapada |
///
/// O teste `os_presets_de_sistema_ficam_dentro_da_escala_do_motor` continua de
/// pé, agora sobre os 53 campos: **preset que sai da faixa pede o que nenhum
/// arrasto de slider consegue pedir**, e é assim que se reconhece o defeito.
///
/// ⚠️ **"Auto" não está aqui, e não é esquecimento.** Ele pedia
/// `exposure: Some(0.0)` com um `// Placeholder` ao lado: clicar não fazia nada,
/// que é a pior das três situações (`docs/PARIDADE-LIGHTROOM.md`). E o lugar
/// dele nunca foi este — no Lightroom "Auto" é um **botão do painel Básico**,
/// que lê a foto e escolhe os tons a partir dela. Um preset é uma lista de
/// números fixos, e nenhuma lista fixa serve para todas as fotos.
pub fn presets_de_sistema() -> Vec<Preset> {
    // 🚨 **As que definem o *look* substituem; as que acrescentam, somam.**
    // Somar é o certo para "Nitidez para impressão" — ela se aplica depois de
    // qualquer tratamento. Não é o certo para as outras seis: a sépia escreve a
    // tonalização, o preto e branco escreve a dessaturação e não desfaz a
    // tonalização, e o que saía era uma foto âmbar com nome de preto e branco
    // (dono, 2026-09-11, na web). Ver `Preset::replaces`.
    let monte = |nome: &str, campos: &[(&str, f32)]| {
        Preset::system_replacing(nome, campos.iter().copied().collect::<PresetAdjustments>())
    };
    let monte_somando = |nome: &str, campos: &[(&str, f32)]| {
        Preset::system(nome, campos.iter().copied().collect::<PresetAdjustments>())
    };

    vec![
        // Cinza é o fator zero, e o fator é `1 + saturation`.
        //
        // 🔑 Só a saturação basta para tirar a cor: a intensidade (`vibrance`)
        // vem **depois** dela no shader e trabalha sobre `canal - luminância`,
        // que num pixel cinza é zero. O que já estava intenso na foto não
        // recolore o preto e branco — por isso o preset não zera a intensidade.
        monte(
            "Preto e branco clássico",
            &[
                ("saturation", -1.0),
                ("contrast", 1.15),
                ("clarity", 0.2),
                ("blacks", -12.0),
                ("whites", 10.0),
            ],
        ),
        // 🚨 **A sépia não existia neste app até 7/set/2026, e o motivo era a
        // ordem do shader.** Temperatura e matiz agem no passo 3, antes da
        // saturação: numa foto em preto e branco eles pintam uma cor que o passo
        // 9 apaga logo depois. O caminho é a tonalização, que entra **depois** —
        // e a tabela de presets só guardava 15 campos, nenhum deles dela.
        //
        // Âmbar por volta de 35° nas sombras e 45° nas altas luzes: é o tom da
        // vitrine do estúdio, e é como se vira uma cópia no quarto escuro.
        monte(
            "Sépia à moda antiga",
            &[
                ("saturation", -1.0),
                ("split_shadow_hue", 35.0),
                ("split_shadow_sat", 45.0),
                ("split_highlight_hue", 45.0),
                ("split_highlight_sat", 30.0),
                ("contrast", 1.08),
                ("highlights", -15.0),
                ("shadows", 12.0),
            ],
        ),
        // Pele mora no laranja e no vermelho: é ali que se ganha viço sem
        // amarelar a roupa e o fundo.
        monte(
            "Retrato suave",
            &[
                ("clarity", -0.25),
                ("contrast", 0.95),
                ("highlights", -20.0),
                ("shadows", 22.0),
                ("hsl_orange_sat", 8.0),
                ("hsl_orange_lum", 10.0),
                ("hsl_red_sat", 5.0),
                ("nr_luminance", 15.0),
                ("sharpen_amount", 25.0),
            ],
        ),
        monte(
            "Luz de estúdio",
            &[
                ("contrast", 1.25),
                ("blacks", -22.0),
                ("whites", 15.0),
                ("clarity", 0.3),
                ("tone_curve_shadows", -12.0),
                ("tone_curve_highlights", 10.0),
            ],
        ),
        // `r += t*10`, `b -= t*10`, em 0..255: 2,5 são 25 níveis para cada lado.
        monte(
            "Hora dourada",
            &[
                ("temperature", 2.5),
                ("exposure", 0.15),
                ("vibrance", 0.25),
                ("highlights", -22.0),
                ("shadows", 15.0),
                ("hsl_yellow_sat", 12.0),
                ("hsl_orange_sat", 10.0),
            ],
        ),
        monte(
            "Alta-chave",
            &[
                ("exposure", 0.5),
                ("contrast", 0.9),
                ("highlights", -12.0),
                ("shadows", 30.0),
                ("blacks", 15.0),
                ("clarity", -0.1),
                ("saturation", -0.1),
            ],
        ),
        // ⚠️ O raio começa em 0,5 porque raio zero não tem pixel de vizinhança —
        // e nitidez sem ruído junto é o que o papel pede.
        //
        // 🔑 **A única que soma**, e é o que ela é: nitidez para o papel não
        // decide a cara da foto — ela se aplica **depois** de qualquer look, e
        // zerar o look para acrescentar nitidez seria o contrário do que o
        // gesto quer dizer.
        monte_somando(
            "Nitidez para impressão",
            &[
                ("sharpen_amount", 55.0),
                ("sharpen_radius", 1.2),
                ("clarity", 0.15),
                ("nr_luminance", 10.0),
            ],
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
