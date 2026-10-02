use domain::entities::preset::PresetAdjustments;
use domain::entities::Preset;
use domain::repositories::PresetRepository;
use domain::DomainResult;
use std::sync::Arc;

/// O **RecordarFotos P&B** — o estilo das fotos vendidas.
///
/// 🚨 Até 2/out/2026 ele era o `.dtstyle` do darktable, com módulos próprios no
/// motor (campos `dt_*`), lendo a foto como sRGB. O dono decidiu um motor só e a
/// leitura em Adobe RGB, e o estilo foi refeito com os controles do Lightroom:
/// `examples/ajustar_pb.rs` procurou estes valores contra o `darktable-cli`
/// 5.6.1 com o mesmo `.dtstyle`, em 6 fotos da câmera — ΔE2000 1,6 a 2,0 (as
/// duas fora do ajuste: 1,74 e 1,87; abaixo de 2 o olho não separa). A borda
/// creme da vinheta vem de a viragem ser aplicada depois dela no processo 1
/// (`corpo.wgsl`, `viragem`), como no Lightroom e no darktable. As fotos guardadas com os `dt_*` antigos migram para estes valores
/// na leitura ([`migrar_do_darktable`]).
pub const RECORDARFOTOS_PB: &[(&str, f32)] = &[
    ("bw_ativo", 1.0),
    ("processo", 1.0),
    ("exposure", 0.29),
    ("contrast", 1.05),
    ("highlights", -2.25),
    ("shadows", 100.0),
    ("whites", -28.25),
    ("blacks", 19.25),
    ("clarity", 0.11),
    ("texture", -6.0),
    ("dehaze", -10.5),
    ("tone_curve_shadows", 23.13),
    ("tone_curve_darks", -5.88),
    ("tone_curve_lights", -6.75),
    ("tone_curve_highlights", -1.13),
    ("bw_red", 1.72),
    ("bw_orange", -0.63),
    ("bw_yellow", 3.75),
    ("bw_green", 26.25),
    ("bw_aqua", 46.25),
    ("bw_blue", 15.0),
    ("bw_magenta", -2.5),
    ("split_shadow_hue", 51.41),
    ("split_shadow_sat", 36.5),
    ("split_highlight_hue", 50.63),
    ("split_highlight_sat", 70.5),
    ("split_balance", -84.69),
    ("lens_vignette_amount", 98.0),
    ("lens_vignette_midpoint", 82.5),
    ("pcv_amount", 43.75),
    ("pcv_midpoint", 28.0),
    ("pcv_feather", 30.0),
    ("pcv_roundness", -62.19),
];

/// A receita guardada (nome → valor) de uma foto revelada com o RecordarFotos
/// P&B do darktable vira a do RecordarFotos P&B de hoje.
///
/// Os campos `dt_*` saíram do motor em 2/out/2026, e todo leitor ignora nome
/// que não conhece: sem isto, a foto vendida reabriria colorida e deixaria de
/// contar como revelada. Quem tem o monocromático do darktable ligado recebe os
/// valores de [`RECORDARFOTOS_PB`] por cima dos seus; os `dt_*` saem sempre.
pub fn migrar_do_darktable(receita: &mut serde_json::Map<String, serde_json::Value>) {
    let era_pb = receita
        .get("dt_monochrome_ativo")
        .and_then(|v| v.as_f64())
        .is_some_and(|v| v >= 0.5);
    receita.retain(|nome, _| !nome.starts_with("dt_"));
    if era_pb {
        for (nome, valor) in RECORDARFOTOS_PB {
            receita.insert((*nome).to_string(), serde_json::json!(valor));
        }
    }
}

/// Os presets que existem antes de alguém salvar o primeiro.
///
/// Eles não moram na tabela `presets`: são construídos aqui a cada listagem, e a
/// tela os separa dos do usuário por `is_system`.
///
/// # 🔑 São os vinte do site, e são os mesmos números
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
    // qualquer tratamento. Não é o certo para as outras sete: a sépia escreve a
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
        // 🎞️ **O estilo das fotos vendidas, refeito com os controles do
        // Lightroom** (dono, 2/out/2026): os valores em [`RECORDARFOTOS_PB`].
        monte("RecordarFotos P&B", RECORDARFOTOS_PB),
        // 🎞️ **Os doze "Vintage ·"** (dono, 2026-09-28) — os mesmos do site
        // (`presets-do-sistema.ts`), campo a campo e na mesma ordem: os
        // clássicos do próprio Lightroom (Aged Photo, Old Polaroid, Yesteryear,
        // Cross Process, Cyanotype, Antique Grayscale, Bleach Bypass, Direct
        // Positive) e quatro emulações de filme, traduzidos pelas conversões do
        // importador de `.xmp`. Temperatura e matiz são os relativos do modo
        // JPEG (sRGB) do Lightroom: a foto do balcão já chega revelada.
        //
        // ⚠️ Os números foram aprovados na POC do site
        // (`public/revelacao/poc-vintage.html`), sobre fotos reais do estúdio no
        // mesmo motor. Todos recomeçam do neutro — o "Zerar os outros ajustes ao
        // aplicar" ligado.
        monte(
            "Vintage · Foto envelhecida",
            &[
                ("contrast", 0.9),
                ("saturation", -0.45),
                ("vibrance", -0.2),
                ("split_shadow_hue", 40.0),
                ("split_shadow_sat", 35.0),
                ("split_highlight_hue", 50.0),
                ("split_highlight_sat", 25.0),
                ("curva_m0", 22.0),
                ("curva_m1", 45.0),
                ("curva_m8", 238.0),
                ("grain_amount", 25.0),
                ("grain_size", 30.0),
                ("lens_vignette_amount", -20.0),
            ],
        ),
        monte(
            "Vintage · Polaroid antiga",
            &[
                ("temperature", 1.5),
                ("contrast", 0.88),
                ("saturation", -0.2),
                ("highlights", -20.0),
                ("split_shadow_hue", 185.0),
                ("split_shadow_sat", 10.0),
                ("split_highlight_hue", 40.0),
                ("split_highlight_sat", 22.0),
                ("curva_m0", 30.0),
                ("curva_m1", 50.0),
                ("curva_m8", 232.0),
                ("lens_vignette_amount", -25.0),
                ("grain_amount", 15.0),
            ],
        ),
        monte(
            "Vintage · Anos passados",
            &[
                ("saturation", -0.35),
                ("clarity", -0.15),
                ("contrast", 0.92),
                ("split_shadow_hue", 30.0),
                ("split_shadow_sat", 20.0),
                ("split_highlight_hue", 45.0),
                ("split_highlight_sat", 18.0),
                ("curva_m0", 25.0),
                ("curva_m1", 42.0),
                ("lens_vignette_amount", -15.0),
            ],
        ),
        monte(
            "Vintage · Processo cruzado",
            &[
                ("contrast", 1.1),
                ("saturation", 0.1),
                ("curva_r2", 54.0),
                ("curva_r6", 200.0),
                ("curva_g2", 58.0),
                ("curva_g6", 197.0),
                ("curva_b0", 24.0),
                ("curva_b2", 70.0),
                ("curva_b6", 184.0),
                ("curva_b8", 222.0),
                ("split_highlight_hue", 55.0),
                ("split_highlight_sat", 12.0),
            ],
        ),
        monte(
            "Vintage · Cianótipo",
            &[
                ("bw_ativo", 1.0),
                ("saturation", -1.0),
                ("contrast", 1.1),
                ("split_shadow_hue", 220.0),
                ("split_shadow_sat", 50.0),
                ("split_highlight_hue", 205.0),
                ("split_highlight_sat", 20.0),
            ],
        ),
        monte(
            "Vintage · Cinza antigo",
            &[
                ("bw_ativo", 1.0),
                ("saturation", -1.0),
                ("contrast", 0.95),
                ("split_shadow_hue", 40.0),
                ("split_shadow_sat", 25.0),
                ("split_highlight_hue", 50.0),
                ("split_highlight_sat", 15.0),
                ("curva_m0", 15.0),
                ("grain_amount", 20.0),
                ("grain_size", 25.0),
                ("lens_vignette_amount", -15.0),
            ],
        ),
        monte(
            "Vintage · Bleach bypass",
            &[
                ("saturation", -0.6),
                ("contrast", 1.35),
                ("clarity", 0.4),
                ("blacks", -20.0),
                ("whites", 10.0),
                ("highlights", -10.0),
            ],
        ),
        monte(
            "Vintage · Positivo direto",
            &[
                ("contrast", 1.3),
                ("saturation", 0.25),
                ("vibrance", 0.2),
                ("blacks", -15.0),
                ("curva_r2", 55.0),
                ("curva_r6", 200.0),
                ("curva_b0", 20.0),
                ("curva_b8", 238.0),
                ("split_highlight_hue", 50.0),
                ("split_highlight_sat", 10.0),
            ],
        ),
        monte(
            "Vintage · Kodachrome",
            &[
                ("contrast", 1.2),
                ("saturation", 0.08),
                ("temperature", 0.5),
                ("hsl_red_sat", 15.0),
                ("hsl_red_hue", -3.0),
                ("hsl_blue_sat", 10.0),
                ("hsl_blue_hue", -6.0),
                ("hsl_green_sat", -15.0),
                ("hsl_green_hue", 6.0),
                ("hsl_yellow_sat", -10.0),
                ("calib_red_sat", 10.0),
                ("calib_blue_sat", 15.0),
                ("split_shadow_hue", 210.0),
                ("split_shadow_sat", 8.0),
                ("curva_m1", 27.0),
                ("curva_m7", 228.0),
            ],
        ),
        monte(
            "Vintage · Portra 400",
            &[
                ("contrast", 0.9),
                ("highlights", -30.0),
                ("shadows", 20.0),
                ("saturation", -0.1),
                ("vibrance", -0.1),
                ("temperature", 0.8),
                ("hsl_orange_sat", -5.0),
                ("hsl_orange_lum", 10.0),
                ("hsl_green_sat", -20.0),
                ("hsl_green_hue", 9.0),
                ("hsl_blue_sat", -15.0),
                ("calib_red_hue", 5.0),
                ("calib_blue_sat", 15.0),
                ("curva_m0", 12.0),
                ("curva_m8", 248.0),
                ("grain_amount", 15.0),
                ("grain_size", 20.0),
            ],
        ),
        monte(
            "Vintage · Ektachrome anos 70",
            &[
                ("contrast", 1.1),
                ("saturation", -0.1),
                ("temperature", -0.3),
                ("shadows", 12.0),
                ("hsl_blue_sat", 15.0),
                ("hsl_aqua_sat", 10.0),
                ("hsl_green_hue", 12.0),
                ("split_shadow_hue", 200.0),
                ("split_shadow_sat", 10.0),
                ("split_highlight_hue", 45.0),
                ("split_highlight_sat", 10.0),
                ("curva_m0", 18.0),
                ("grain_amount", 18.0),
            ],
        ),
        monte(
            "Vintage · Desbotado anos 70",
            &[
                ("temperature", 2.0),
                ("saturation", -0.25),
                ("contrast", 0.9),
                ("curva_m0", 35.0),
                ("curva_m1", 52.0),
                ("curva_m8", 230.0),
                ("curva_b0", 45.0),
                ("split_shadow_hue", 35.0),
                ("split_shadow_sat", 25.0),
                ("split_highlight_hue", 45.0),
                ("split_highlight_sat", 15.0),
                ("grain_amount", 20.0),
                ("lens_vignette_amount", -20.0),
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
        // 🎬 **O DNG do Estúdio Canela** (dono, 2026-09-30: *"crie um preset
        // padrão chamado de Cinematográfico P&B baseado nesse"*): a "P&B
        // Cinematografico" do Lightroom com a "Vinheta Borda" (a moldura branca
        // de cantos redondos) e a exposição daquele DNG, sem o corte. Os
        // números são os que a importação tira do XMP dele.
        monte(
            "Cinematográfico P&B",
            &[
                ("exposure", 0.14),
                ("highlights", -58.0),
                ("shadows", 80.0),
                ("blacks", 80.0),
                ("texture", 12.0),
                ("clarity", 0.35),
                ("dehaze", 15.0),
                ("saturation", -1.0),
                ("curva_m0", 37.0),
                ("curva_m1", 37.0),
                ("curva_m2", 60.1752),
                ("curva_m3", 92.646),
                ("curva_m4", 125.1168),
                ("curva_m5", 157.5876),
                ("curva_m6", 190.0584),
                ("curva_m7", 222.5292),
                ("sharpen_amount", 25.3333),
                ("sharpen_radius", 2.1),
                ("sharpen_detail", 100.0),
                ("sharpen_masking", 90.0),
                ("nr_color", 25.0),
                ("split_shadow_hue", 14.0),
                ("split_shadow_sat", 32.0),
                ("split_shadow_lum", -13.0),
                ("split_midtone_hue", 44.0),
                ("split_midtone_sat", 39.0),
                ("split_highlight_hue", 59.0),
                ("split_highlight_sat", 24.0),
                ("split_global_hue", 221.0),
                ("split_global_sat", 18.0),
                ("pcv_style", 2.0),
                ("pcv_amount", 100.0),
                ("pcv_midpoint", 0.0),
                ("pcv_roundness", -83.0),
                ("pcv_feather", 35.0),
            ],
        ),
    ]
    .into_iter()
    .chain(presets_do_lightroom())
    .collect()
}

/// O nome da pasta das predefinições do Lightroom do estúdio.
pub const GRUPO_LRS: &str = "LRs";

/// Uma predefinição do Lightroom, como `lightroom.json` a guarda.
#[derive(serde::Deserialize)]
struct DoLightroom {
    nome: String,
    recomeca: bool,
    ajustes: std::collections::BTreeMap<String, f32>,
}

/// 🎞️ **As predefinições do Lightroom do estúdio, na pasta "LRs"** (dono,
/// 2026-09-30: *"crie um grupo novo de presets chamado LRs com todos esses
/// efeitos, sendo que alguns precisam resetar o anterior e não somar"*).
///
/// `lightroom.json` sai dos `.xmp` do estúdio pelo mesmo tradutor da
/// importação (`infrastructure/examples/presets_do_lightroom.rs`), e o site lê
/// o mesmo arquivo. **As de vinheta somam** — vão por cima do visual que já
/// está na foto, como no Lightroom —; **as outras recomeçam do neutro**, porque
/// são o visual inteiro.
pub fn presets_do_lightroom() -> Vec<Preset> {
    let lista: Vec<DoLightroom> = serde_json::from_str(include_str!("lightroom.json"))
        .expect("lightroom.json é gerado pelo exemplo e vai junto no repositório");
    lista
        .into_iter()
        .map(|p| {
            let ajustes: PresetAdjustments = p.ajustes.into_iter().collect();
            let preset = if p.recomeca {
                Preset::system_replacing(&p.nome, ajustes)
            } else {
                Preset::system(&p.nome, ajustes)
            };
            preset.no_grupo(GRUPO_LRS)
        })
        .collect()
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
        // Uma predefinição importada de um `.dtstyle` antes de 2/out/2026 só
        // tem campos `dt_*`: a do P&B vira o RecordarFotos P&B de hoje, e as
        // outras perdem só o que o motor não tem mais.
        for preset in &mut user_presets {
            if preset.adjustments.campos().any(|c| c.starts_with("dt_")) {
                let mut receita: serde_json::Map<String, serde_json::Value> = preset
                    .adjustments
                    .iter()
                    .map(|(k, v)| (k.to_string(), serde_json::json!(v)))
                    .collect();
                migrar_do_darktable(&mut receita);
                preset.adjustments = receita
                    .iter()
                    .filter_map(|(k, v)| Some((k.as_str(), v.as_f64()? as f32)))
                    .collect();
                // Um estilo do darktable é um visual inteiro: recomeça do neutro.
                preset.replaces = true;
            }
        }

        let mut all_presets = presets_de_sistema();
        all_presets.append(&mut user_presets);

        Ok(all_presets)
    }
}
