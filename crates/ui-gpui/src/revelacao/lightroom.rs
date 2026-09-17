//! Ler predefinições do Lightroom — `.lrtemplate` e `.xmp` — e, pela porta de
//! [`darktable`], os estilos do darktable (`.dtstyle` e o `.xmp` dele).
//!
//! É o porte de `revelacao/lightroom.ts` do site, e existe pelo mesmo motivo:
//! quem revela tem uma coleção de presets comprados, e recriá-los slider a
//! slider não é trabalho que alguém faça.
//!
//! # 🚨 As escalas do Lightroom não são as deste motor
//!
//! Este é o ponto que já custou meses aqui — os quatro presets de sistema
//! passaram meses pedindo `saturation: -100` numa escala que vai de -1 a 1.
//! `Contrast2012` vai de -100 a 100 e o `contrast` daqui é um **multiplicador**
//! de 0 a 2 com neutro em 1; `Saturation` vai de -100 a 100 e o daqui vai de -1
//! a 1, onde -1 é cinza. Copiar o número sem converter não dá erro — dá uma foto
//! destruída que parece decisão de cor.
//!
//! # O que este motor não tem
//!
//! Remoção de névoa, textura e as correções de lente por perfil. Por isso a
//! importação **conta o que ignorou**, por arquivo: dizer "importado" e entregar
//! outra imagem seria o pior desfecho — o operador procuraria defeito no próprio
//! olho.
//!
//! ✅ **A curva por ponto, a calibração de câmera, os tons médios e o global do
//! Color Grading e o mixer de preto e branco saíram desta lista** — no site em
//! 2026-09-12, aqui em 17/set/2026, quando esta tradução alcançou a de lá. Numa
//! amostra de 400 presets comerciais, **321 usavam curva por ponto**: o preset
//! chegava sem o pé de contraste que o define.
//!
//! ✅ **Split toning e grão também não estão na lista**, desde 6/set/2026. Eram
//! **342 dos mesmos 400**.
//!
//! # ⚠️ Sem `regex`, e de propósito
//!
//! O original em TypeScript usa expressões regulares; aqui elas seriam uma
//! dependência nova no workspace para varrer quatro padrões que cabem num
//! `char_indices`. O que se lê é sempre o mesmo formato: `chave = valor` no
//! primeiro nível de uma tabela.

use std::collections::BTreeMap;

use domain::entities::preset::PresetAdjustments;

use super::processador::Ajustes;

/// Os estilos do darktable (`darktable.ts`).
pub mod darktable;

/// Um valor lido do arquivo, antes de traduzir.
///
/// 🔑 **Texto e tabela existem só para o aviso.** A tradução usa números e o
/// `ConvertToGrayscale`; os outros dois estão aqui para `mexe_na_imagem` poder
/// dizer se um recurso sem equivalente estava **ligado** — um `ToneCurve` com
/// valor `"Linear"` é a curva neutra, e avisar sobre ele seria alarme falso.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Numero(f32),
    Booleano(bool),
    Texto(String),
    Tabela(Vec<String>),
}

impl Valor {
    fn numero(&self) -> Option<f32> {
        match self {
            Valor::Numero(v) if v.is_finite() => Some(*v),
            _ => None,
        }
    }

    /// Zero e `false` não mudam nada — não valem como "ignorado".
    fn mexe_na_imagem(&self) -> bool {
        match self {
            Valor::Numero(v) => *v != 0.0,
            Valor::Booleano(v) => *v,
            Valor::Tabela(itens) => !itens.is_empty(),
            Valor::Texto(texto) => !texto.is_empty() && texto != "Linear",
        }
    }
}

/// O que se extraiu de um arquivo, antes de traduzir.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetBruto {
    pub nome: String,
    pub ajustes: BTreeMap<String, Valor>,
}

/// Uma predefinição traduzida, com o que ficou pelo caminho.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetTraduzido {
    pub nome: String,
    pub ajustes: PresetAdjustments,
    /// Recursos que mudam a imagem e este motor não tem, sem repetição.
    ///
    /// Texto próprio, e não `&'static str`: o darktable diz **qual** módulo e
    /// **qual** versão ficou de fora.
    pub ignorados: Vec<String>,
}

/// Do número do Lightroom para o do shader, com a faixa do slider.
struct Conversao {
    campo: &'static str,
    minimo: f32,
    maximo: f32,
    converter: fn(f32) -> f32,
}

fn direto(campo: &'static str, minimo: f32, maximo: f32) -> Conversao {
    Conversao {
        campo,
        minimo,
        maximo,
        converter: |v| v,
    }
}

/// -100..100 do Lightroom para -1..1 daqui (textura, intensidade, saturação).
fn por_cem(campo: &'static str) -> Conversao {
    Conversao {
        campo,
        minimo: -1.0,
        maximo: 1.0,
        converter: |v| v / 100.0,
    }
}

/// ⚠️ O matiz do Lightroom vai de -100 a 100 e o daqui em **graus**, de -180 a
/// 180. Não são a mesma escala: o extremo do slider da Adobe desloca cerca de
/// 30°, não meia volta. O fator é uma aproximação declarada — mapear
/// proporcionalmente ao intervalo giraria a cor muito além do que o preset pede.
const GRAUS_POR_PONTO_DE_MATIZ: f32 = 0.3;

/// As oito cores, no nome da Adobe e no nosso.
const CORES: [(&str, &str); 8] = [
    ("Red", "red"),
    ("Orange", "orange"),
    ("Yellow", "yellow"),
    ("Green", "green"),
    ("Aqua", "aqua"),
    ("Blue", "blue"),
    ("Purple", "purple"),
    ("Magenta", "magenta"),
];

/// O que cada chave do Lightroom vira aqui.
fn conversao(chave: &str) -> Option<Conversao> {
    // O HSL é 24 chaves com o nome da cor no meio — resolvido por sufixo, e não
    // por 24 linhas de tabela.
    for (deles, nosso) in CORES {
        if let Some(familia) = chave.strip_suffix(deles) {
            return match familia {
                "SaturationAdjustment" => Some(Conversao {
                    campo: campo_do_motor(&format!("hsl_{nosso}_sat")),
                    minimo: -100.0,
                    maximo: 100.0,
                    converter: |v| v,
                }),
                "LuminanceAdjustment" => Some(Conversao {
                    campo: campo_do_motor(&format!("hsl_{nosso}_lum")),
                    minimo: -100.0,
                    maximo: 100.0,
                    converter: |v| v,
                }),
                "HueAdjustment" => Some(Conversao {
                    campo: campo_do_motor(&format!("hsl_{nosso}_hue")),
                    minimo: -180.0,
                    maximo: 180.0,
                    converter: |v| v * GRAUS_POR_PONTO_DE_MATIZ,
                }),
                // ⚠️ **Estes oito só agem com `bw_ativo`**, que
                // `ConvertToGrayscale` liga (ver [`traduzir`]). Um preset de cor
                // que traga `GrayMixer` dentro — e há — não pode dessaturar a
                // foto de ninguém.
                "GrayMixer" => Some(direto(
                    campo_do_motor(&format!("bw_{nosso}")),
                    -100.0,
                    100.0,
                )),
                _ => None,
            };
        }
    }

    Some(match chave {
        // ------------------------------------------------------------ Básico
        // A exposição é a única que já vem na mesma unidade: pontos de luz.
        "Exposure2012" | "Exposure" => direto("exposure", -5.0, 5.0),
        // 🚨 Multiplicador, não porcentagem. Neutro 1, faixa 0..2.
        "Contrast2012" | "Contrast" => Conversao {
            campo: "contrast",
            minimo: 0.0,
            maximo: 2.0,
            converter: |v| 1.0 + v / 100.0,
        },
        "Highlights2012" => direto("highlights", -100.0, 100.0),
        "Shadows2012" => direto("shadows", -100.0, 100.0),
        "Whites2012" => direto("whites", -100.0, 100.0),
        "Blacks2012" => direto("blacks", -100.0, 100.0),
        "Clarity2012" | "Clarity" => por_cem("clarity"),
        "Vibrance" => por_cem("vibrance"),
        "Saturation" => por_cem("saturation"),
        // ------------------------------------------------- Curva paramétrica
        "ParametricShadows" => direto("tone_curve_shadows", -100.0, 100.0),
        "ParametricDarks" => direto("tone_curve_darks", -100.0, 100.0),
        "ParametricLights" => direto("tone_curve_lights", -100.0, 100.0),
        "ParametricHighlights" => direto("tone_curve_highlights", -100.0, 100.0),
        // ----------------------------------------------------------- Detalhe
        "LuminanceSmoothing" => direto("nr_luminance", 0.0, 100.0),
        "ColorNoiseReduction" => direto("nr_color", 0.0, 100.0),
        // A nitidez da Adobe vai a 150; a daqui, a 100.
        "Sharpness" => Conversao {
            campo: "sharpen_amount",
            minimo: 0.0,
            maximo: 100.0,
            converter: |v| v * 100.0 / 150.0,
        },
        "SharpenRadius" => direto("sharpen_radius", 0.5, 3.0),
        // ------------------------------------------------------------- Lente
        // A vinheta **pós-corte** é a que o preset usa como efeito; a de lente
        // (`VignetteAmount`) é correção óptica e some quando há corte.
        "PostCropVignetteAmount" => direto("lens_vignette_amount", -100.0, 100.0),
        "PostCropVignetteMidpoint" => direto("lens_vignette_midpoint", 0.0, 100.0),
        "LensManualDistortionAmount" => direto("lens_distortion", -100.0, 100.0),
        // ------------------------------------------------------- Tonalização
        // 🔑 As escalas batem, e é por sorte: matiz em graus na roda inteira e
        // saturação de 0 a 100 nos dois lados. O recorte fica assim mesmo — um
        // preset com matiz 400 é arquivo estragado, não escala diferente.
        "SplitToningShadowHue" | "ColorGradeShadowHue" => direto("split_shadow_hue", 0.0, 360.0),
        "SplitToningShadowSaturation" | "ColorGradeShadowSat" => {
            direto("split_shadow_sat", 0.0, 100.0)
        }
        "SplitToningHighlightHue" | "ColorGradeHighlightHue" => {
            direto("split_highlight_hue", 0.0, 360.0)
        }
        "SplitToningHighlightSaturation" | "ColorGradeHighlightSat" => {
            direto("split_highlight_sat", 0.0, 100.0)
        }
        "SplitToningBalance" => direto("split_balance", -100.0, 100.0),
        // ------------------------------- Color Grading: os eixos que faltavam
        "ColorGradeMidtoneHue" => direto("split_midtone_hue", 0.0, 360.0),
        "ColorGradeMidtoneSat" => direto("split_midtone_sat", 0.0, 100.0),
        "ColorGradeGlobalHue" => direto("split_global_hue", 0.0, 360.0),
        "ColorGradeGlobalSat" => direto("split_global_sat", 0.0, 100.0),
        // 🔑 A mistura da Adobe já vem em 0–100 e o neutro dela é 50, igual ao
        // daqui.
        "ColorGradeBlending" => direto("split_blending", 0.0, 100.0),
        // -------------------------------------------- Calibração de câmera
        // 🚨 **A base da maioria dos presets de filme.** As escalas batem: matiz
        // e saturação de −100 a 100 nos dois lados. O que muda é o significado
        // do extremo — a daqui é rotação de matiz por primário, a da Adobe é
        // matriz no espaço do perfil da câmera.
        "RedHue" => direto("calib_red_hue", -100.0, 100.0),
        "RedSaturation" => direto("calib_red_sat", -100.0, 100.0),
        "GreenHue" => direto("calib_green_hue", -100.0, 100.0),
        "GreenSaturation" => direto("calib_green_sat", -100.0, 100.0),
        "BlueHue" => direto("calib_blue_hue", -100.0, 100.0),
        "BlueSaturation" => direto("calib_blue_sat", -100.0, 100.0),
        "ShadowTint" => direto("calib_shadow_tint", -100.0, 100.0),
        // -------------------------------------------------------------- Grão
        "GrainAmount" => direto("grain_amount", 0.0, 100.0),
        "GrainSize" => direto("grain_size", 0.0, 100.0),
        _ => return None,
    })
}

/// O nome de um campo do motor, como `&'static str`.
///
/// O `PresetAdjustments` guarda `String`, mas a tabela de conversões é de
/// `&'static str` — e procurar o nome montado em `Ajustes::NOMES` é o que faz um
/// erro de digitação (`hsl_orenge_sat`) derrubar o teste em vez de virar um
/// preset que importa "com sucesso" e não move nada.
fn campo_do_motor(nome: &str) -> &'static str {
    Ajustes::NOMES
        .iter()
        .find(|campo| **campo == nome)
        .copied()
        .unwrap_or_else(|| unreachable!("`{nome}` não está em `Ajustes::NOMES`"))
}

/// O que muda a imagem no Lightroom e **não existe** neste motor.
///
/// 🔑 A lista é de **prefixos**, e é o que alimenta o aviso da tela. Sem ela, o
/// operador importa um preset de filme — que é split toning e curva —, vê quase
/// nada acontecer e conclui que o importador está quebrado, quando o que
/// aconteceu foi o motor não ter o efeito.
///
/// ⚠️ **Prefixo, e não o nome da família inteira.** "SplitToning", "Grain" e
/// "ColorGrade" saíram daqui quando o motor ganhou tonalização e grão; o que
/// ficou tem de ser específico o bastante para não engolir a chave que hoje é
/// traduzida — `GrainAmount` começa com "Grain".
const SEM_EQUIVALENTE: &[(&str, &str)] = &[
    ("GrainFrequency", "aspereza do grão"),
    ("Dehaze", "remoção de névoa"),
    ("Texture", "textura (LR moderno)"),
    ("Defringe", "correção de franjas"),
    ("ChromaticAberration", "aberração cromática"),
    ("SharpenDetail", "detalhe da nitidez"),
    ("SharpenEdgeMasking", "máscara de bordas"),
    ("LuminanceNoiseReduction", "detalhe do ruído"),
    ("ColorNoiseReductionDetail", "detalhe do ruído de cor"),
    ("ColorNoiseReductionSmoothness", "suavidade do ruído de cor"),
    ("PostCropVignetteRoundness", "forma da vinheta"),
    ("PostCropVignetteFeather", "suavidade da vinheta"),
    ("PostCropVignetteStyle", "estilo da vinheta"),
    (
        "PostCropVignetteHighlightContrast",
        "vinheta nas altas luzes",
    ),
    ("Temperature", "temperatura (depende do balanço da foto)"),
    ("Tint", "matiz do balanço de branco"),
];

/// De que chave do arquivo sai cada canal da curva por ponto.
///
/// 🔑 **`ToneCurvePV2012` é a do processo 2012 e `ToneCurve` a antiga**, e as
/// duas existem no mesmo arquivo quando o preset foi salvo por uma versão de
/// transição. A nova manda; a velha só entra quando a nova não veio — o mesmo
/// critério do `Exposure2012` contra o `Exposure`.
const CURVAS_DO_LIGHTROOM: [(char, [&str; 2]); 4] = [
    ('m', ["ToneCurvePV2012", "ToneCurve"]),
    ('r', ["ToneCurvePV2012Red", "ToneCurveRed"]),
    ('g', ["ToneCurvePV2012Green", "ToneCurveGreen"]),
    ('b', ["ToneCurvePV2012Blue", "ToneCurveBlue"]),
];

/// Quantos pontos a curva por ponto tem, por canal (`curva_m0`…`curva_m8`).
const PONTOS_DA_CURVA: usize = 9;

/// A altura do ponto `i` da curva neutra: a reta `y = x`.
fn neutro_da_curva(i: usize) -> f32 {
    i as f32 * 255.0 / (PONTOS_DA_CURVA - 1) as f32
}

/// Esta curva devolve a foto como ela entrou?
fn curva_eh_neutra(alturas: &[f32]) -> bool {
    alturas
        .iter()
        .enumerate()
        .all(|(i, v)| (v - neutro_da_curva(i)).abs() < 0.01)
}

/// Os pontos de uma `ToneCurvePV2012`, já em números — o
/// `lerPontosDoLightroom` do site.
///
/// # 🚨 São dois formatos, e o mesmo campo
///
/// Uma lista de textos `"x, y"` (um ponto por item), ou a lista **plana** do
/// `.lrtemplate` — `{0, 22, 254, 202}` são os pontos (0, 22) e (254, 202). É a
/// vírgula dentro do item que separa os dois. Ponto ilegível é descartado em vez
/// de derrubar o preset.
fn pontos_da_curva(itens: &[String]) -> Vec<(f32, f32)> {
    let preso = |v: f32| v.clamp(0.0, 255.0);
    let numero = |t: &str| t.trim().parse::<f32>().ok().filter(|v| v.is_finite());
    let mut pontos: Vec<(f32, f32)> = if itens.iter().any(|item| item.contains(',')) {
        itens
            .iter()
            .filter_map(|item| {
                let mut partes = item.split(',');
                let x = numero(partes.next()?)?;
                let y = numero(partes.next()?)?;
                Some((preso(x), preso(y)))
            })
            .collect()
    } else {
        itens
            .as_chunks::<2>()
            .0
            .iter()
            .filter_map(|[x, y]| Some((preso(numero(x)?), preso(numero(y)?))))
            .collect()
    };
    pontos.sort_by(|a, b| a.0.total_cmp(&b.0));
    pontos
}

/// Reduz a curva de pontos livres do Lightroom às nove alturas daqui — o
/// `daCurvaDoLightroom` do site.
///
/// Amostrar a curva dele nos nossos nove x é a redução honesta: o traço geral se
/// mantém, e o que se perde é uma inflexão mais estreita que 32 níveis. ⚠️
/// **Menos de dois pontos não é curva**: devolve a identidade em vez de uma reta
/// horizontal, que apagaria a foto.
fn da_curva_do_lightroom(pontos: &[(f32, f32)]) -> [f32; PONTOS_DA_CURVA] {
    let mut alturas = [0.0; PONTOS_DA_CURVA];
    for (i, altura) in alturas.iter_mut().enumerate() {
        *altura = if pontos.len() < 2 {
            neutro_da_curva(i)
        } else {
            avaliar_entre_pontos(pontos, neutro_da_curva(i))
        };
    }
    alturas
}

/// A curva do Lightroom em `x`, pelos pontos dele: Hermite monótono
/// (Fritsch–Carlson), a mesma interpolação do shader. Fora do intervalo dos
/// pontos ela gruda na ponta, que é o que o Lightroom faz.
fn avaliar_entre_pontos(pontos: &[(f32, f32)], x: f32) -> f32 {
    let n = pontos.len();
    let (xs, ys): (Vec<f32>, Vec<f32>) = pontos.iter().copied().unzip();
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let mut i = 0;
    while i < n - 2 && x > xs[i + 1] {
        i += 1;
    }
    let h = xs[i + 1] - xs[i];
    if h <= 0.0 {
        return ys[i];
    }
    let t = (x - xs[i]) / h;
    let d = (ys[i + 1] - ys[i]) / h;
    let d_anterior = if i > 0 {
        (ys[i] - ys[i - 1]) / (xs[i] - xs[i - 1])
    } else {
        d
    };
    let d_proximo = if i < n - 2 {
        (ys[i + 2] - ys[i + 1]) / (xs[i + 2] - xs[i + 1])
    } else {
        d
    };
    let m0 = if d_anterior * d > 0.0 {
        2.0 * d_anterior * d / (d_anterior + d)
    } else {
        0.0
    };
    let m1 = if d * d_proximo > 0.0 {
        2.0 * d * d_proximo / (d + d_proximo)
    } else {
        0.0
    };
    let t2 = t * t;
    let t3 = t2 * t;
    let y = (2.0 * t3 - 3.0 * t2 + 1.0) * ys[i]
        + (t3 - 2.0 * t2 + t) * m0 * h
        + (-2.0 * t3 + 3.0 * t2) * ys[i + 1]
        + (t3 - t2) * m1 * h;
    y.clamp(0.0, 255.0)
}

/// Traduz um preset bruto.
///
/// `ConvertToGrayscale` acende o mixer de preto e branco **e** leva a saturação
/// a −1. Os dois: o mixer decide a luminância de cada faixa de matiz, e a
/// dessaturação tira a cor. Só o segundo dava cinza chapado, sem a mistura por
/// canal que separa um P&B de retrato de um P&B sem graça.
pub fn traduzir(bruto: &PresetBruto) -> PresetTraduzido {
    let neutro = Ajustes::default();
    let neutro = neutro.como_vetor();
    let mut ajustes = PresetAdjustments::vazia();
    let mut ignorados: Vec<String> = Vec::new();

    // 🚨 **A curva por ponto, que era o maior buraco deste importador.** ⚠️ A
    // curva neutra não é gravada: muitos presets trazem `0,0 → 255,255` mesmo
    // intocada, e ela não deve "reencostar" a curva de quem aplicar.
    for (canal, chaves) in CURVAS_DO_LIGHTROOM {
        let Some(itens) = chaves
            .iter()
            .find_map(|chave| match bruto.ajustes.get(*chave) {
                Some(Valor::Tabela(itens)) => Some(itens),
                _ => None,
            })
        else {
            continue;
        };
        let alturas = da_curva_do_lightroom(&pontos_da_curva(itens));
        if curva_eh_neutra(&alturas) {
            continue;
        }
        for (i, altura) in alturas.iter().enumerate() {
            ajustes = ajustes.com(format!("curva_{canal}{i}"), arredondar(*altura));
        }
    }

    for (chave, valor) in &bruto.ajustes {
        if CURVAS_DO_LIGHTROOM
            .iter()
            .any(|(_, chaves)| chaves.contains(&chave.as_str()))
        {
            continue;
        }
        if chave == "ConvertToGrayscale" && *valor == Valor::Booleano(true) {
            ajustes = ajustes.com("bw_ativo", 1.0).com("saturation", -1.0);
            continue;
        }

        if let Some(conversao) = conversao(chave) {
            let Some(bruto) = valor.numero() else {
                continue;
            };
            let convertido =
                arredondar((conversao.converter)(bruto).clamp(conversao.minimo, conversao.maximo));
            // ⚠️ **Campo no neutro não é ajuste**: gravá-lo faria a predefinição
            // "reencostar" o controle em vez de deixá-lo como está — e uma de
            // nitidez apagaria o contraste da foto ao ser aplicada.
            let posicao = Ajustes::NOMES
                .iter()
                .position(|nome| *nome == conversao.campo);
            if posicao.is_some_and(|i| convertido != neutro[i]) {
                ajustes = ajustes.com(conversao.campo, convertido);
            }
            continue;
        }

        if !valor.mexe_na_imagem() {
            continue;
        }
        if let Some((_, rotulo)) = SEM_EQUIVALENTE
            .iter()
            .find(|(prefixo, _)| chave.starts_with(prefixo))
        {
            if !ignorados.iter().any(|i| i == rotulo) {
                ignorados.push(rotulo.to_string());
            }
        }
    }

    ignorados.sort_unstable();
    PresetTraduzido {
        nome: bruto.nome.clone(),
        ajustes,
        ignorados,
    }
}

/// Quatro casas bastam para um slider, e encurtam o que vai ao banco.
fn arredondar(v: f32) -> f32 {
    (v * 10_000.0).round() / 10_000.0
}

/// Escolhe o leitor pela extensão, e cai para o conteúdo quando ela mente.
pub fn ler_arquivo(texto: &str, nome_do_arquivo: &str) -> Option<PresetBruto> {
    let minusculo = nome_do_arquivo.to_lowercase();
    if minusculo.ends_with(".xmp") {
        return ler_xmp(texto, nome_do_arquivo);
    }
    if minusculo.ends_with(".lrtemplate") {
        return ler_lrtemplate(texto, nome_do_arquivo);
    }
    ler_lrtemplate(texto, nome_do_arquivo).or_else(|| ler_xmp(texto, nome_do_arquivo))
}

/// Lê um `.lrtemplate` — o formato antigo do Lightroom, que é uma tabela Lua.
///
/// ⚠️ **Não é JSON, e não vale tentar convertê-lo em um**: as chaves não têm
/// aspas, o separador final é vírgula e há tabelas soltas (`{ 0, 0, 255 }`). O
/// que se lê aqui é só o bloco `settings`, par a par, no primeiro nível — é tudo
/// o que a tradução usa, e cada linha a mais seria superfície para errar.
pub fn ler_lrtemplate(texto: &str, nome_do_arquivo: &str) -> Option<PresetBruto> {
    let bloco = bloco_balanceado(texto, "settings")?;
    Some(PresetBruto {
        nome: titulo_do_lrtemplate(texto).unwrap_or_else(|| sem_extensao(nome_do_arquivo)),
        ajustes: pares_lua(bloco),
    })
}

/// O nome que o preset traz dentro, quando traz.
///
/// O `title` pode ser uma string ou uma tabela de traduções (`{ z = "Nome" }`),
/// que é como o Lightroom guarda preset localizado.
fn titulo_do_lrtemplate(texto: &str) -> Option<String> {
    for chave in ["title", "internalName"] {
        let depois = depois_de_chave(texto, chave)?;
        let valor = match depois.chars().next()? {
            '"' => primeira_string(depois),
            '{' => primeira_string(depois),
            _ => None,
        };
        if let Some(valor) = valor {
            let valor = valor.trim().to_string();
            if !valor.is_empty() {
                return Some(valor);
            }
        }
    }
    None
}

/// O texto depois de `chave =`, pulando o espaço.
fn depois_de_chave<'a>(texto: &'a str, chave: &str) -> Option<&'a str> {
    let mut resto = texto;
    while let Some(i) = resto.find(chave) {
        let antes_ok = i == 0
            || !resto[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
        let depois = resto[i + chave.len()..].trim_start();
        if antes_ok && depois.starts_with('=') {
            return Some(depois[1..].trim_start());
        }
        resto = &resto[i + chave.len()..];
    }
    None
}

/// O conteúdo da primeira `"…"` do trecho.
fn primeira_string(texto: &str) -> Option<String> {
    let abre = texto.find('"')?;
    let resto = &texto[abre + 1..];
    let fecha = resto.find('"')?;
    Some(resto[..fecha].to_string())
}

/// O conteúdo de `chave = { … }`, contando as chaves para achar o fim.
fn bloco_balanceado<'a>(texto: &'a str, chave: &str) -> Option<&'a str> {
    let depois = depois_de_chave(texto, chave)?;
    if !depois.starts_with('{') {
        return None;
    }
    let inicio = texto.len() - depois.len();
    let mut profundidade = 0usize;
    for (i, c) in texto[inicio..].char_indices() {
        match c {
            '{' => profundidade += 1,
            '}' => {
                profundidade -= 1;
                if profundidade == 0 {
                    return Some(&texto[inicio + 1..inicio + i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Os pares `Chave = valor` do primeiro nível de uma tabela Lua.
fn pares_lua(bloco: &str) -> BTreeMap<String, Valor> {
    let mut saida = BTreeMap::new();
    let bytes = bloco.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        // O nome: uma palavra que começa por letra ou `_`.
        while i < bytes.len() && !(bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
            i += 1;
        }
        let inicio = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        if inicio == i {
            break;
        }
        let nome = bloco[inicio..i].to_string();

        // O `=`, se houver. Sem ele isto não era um par — segue adiante.
        let depois = i;
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            i = depois;
            continue;
        }
        i += 1;
        while i < bytes.len() && (bytes[i] as char).is_whitespace() {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }

        match bytes[i] {
            // Tabela aninhada: guarda como lista, só para o aviso saber que
            // havia algo, e pula até o fecho.
            b'{' => {
                let abre = i;
                let mut profundidade = 0usize;
                while i < bytes.len() {
                    match bytes[i] {
                        b'{' => profundidade += 1,
                        b'}' => {
                            profundidade -= 1;
                            if profundidade == 0 {
                                i += 1;
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let dentro = &bloco[abre + 1..i.saturating_sub(1)];
                saida.insert(
                    nome,
                    Valor::Tabela(
                        dentro
                            .split(',')
                            .map(str::trim)
                            .filter(|p| !p.is_empty())
                            .map(str::to_string)
                            .collect(),
                    ),
                );
            }
            b'"' => {
                let abre = i;
                i += 1;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += 1;
                }
                let texto = bloco[abre + 1..i.min(bloco.len())].to_string();
                if i < bytes.len() {
                    i += 1;
                }
                saida.insert(nome, Valor::Texto(texto));
            }
            _ => {
                let abre = i;
                while i < bytes.len() && !matches!(bytes[i], b',' | b'\n' | b'}') {
                    i += 1;
                }
                saida.insert(nome, valor_cru(bloco[abre..i].trim()));
            }
        }
    }

    saida
}

/// Lê um `.xmp` — o formato atual, XML com o namespace `crs`.
///
/// Os valores aparecem de dois jeitos conforme quem gravou: como atributo
/// (`crs:Exposure2012="0.5"`) ou como elemento
/// (`<crs:Exposure2012>0.5</crs:Exposure2012>`). Os dois são lidos, e o elemento
/// vence — é o que o Lightroom recente escreve.
pub fn ler_xmp(texto: &str, nome_do_arquivo: &str) -> Option<PresetBruto> {
    if !texto.contains("crs:") {
        return None;
    }
    let mut ajustes = BTreeMap::new();

    for (chave, valor) in atributos_crs(texto) {
        ajustes.insert(chave, valor_cru(&valor));
    }
    for (chave, valor) in elementos_crs(texto) {
        ajustes.insert(chave, valor_cru(valor.trim()));
    }

    let nome = nome_do_xmp(texto).unwrap_or_else(|| sem_extensao(nome_do_arquivo));
    Some(PresetBruto { nome, ajustes })
}

/// `crs:Chave="valor"`, em qualquer lugar do arquivo.
fn atributos_crs(texto: &str) -> Vec<(String, String)> {
    let mut saida = Vec::new();
    let mut resto = texto;

    while let Some(i) = resto.find("crs:") {
        let depois = &resto[i + 4..];
        let fim_do_nome = depois
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(depois.len());
        let chave = &depois[..fim_do_nome];
        let cauda = depois[fim_do_nome..].trim_start();
        if !chave.is_empty() && cauda.starts_with('=') {
            if let Some(valor) = primeira_string(&cauda[1..]) {
                saida.push((chave.to_string(), valor));
            }
        }
        resto = &resto[i + 4..];
    }

    saida
}

/// `<crs:Chave>valor</crs:Chave>`.
fn elementos_crs(texto: &str) -> Vec<(String, String)> {
    let mut saida = Vec::new();
    let mut resto = texto;

    while let Some(i) = resto.find("<crs:") {
        let depois = &resto[i + 5..];
        let fim_do_nome = depois
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(depois.len());
        let chave = depois[..fim_do_nome].to_string();
        let cauda = &depois[fim_do_nome..];
        if !chave.is_empty() && cauda.starts_with('>') {
            let fecho = format!("</crs:{chave}>");
            if let Some(fim) = cauda.find(&fecho) {
                saida.push((chave, cauda[1..fim].to_string()));
            }
        }
        resto = &resto[i + 5..];
    }

    saida
}

/// O nome que o `.xmp` traz: atributo, ou a primeira alternativa da lista RDF.
fn nome_do_xmp(texto: &str) -> Option<String> {
    for (chave, valor) in atributos_crs(texto) {
        if chave == "PresetName" && !valor.trim().is_empty() {
            return Some(valor.trim().to_string());
        }
    }
    for (chave, valor) in elementos_crs(texto) {
        if chave == "PresetName" || chave == "Name" {
            if let Some(nome) = item_rdf(&valor) {
                return Some(nome);
            }
        }
    }
    None
}

/// O conteúdo do primeiro `<rdf:li …>…</rdf:li>`.
fn item_rdf(texto: &str) -> Option<String> {
    let abre = texto.find("<rdf:li")?;
    let depois = &texto[abre..];
    let fim_da_tag = depois.find('>')?;
    let fecha = depois.find("</rdf:li>")?;
    let nome = depois[fim_da_tag + 1..fecha].trim().to_string();
    (!nome.is_empty()).then_some(nome)
}

fn valor_cru(cru: &str) -> Valor {
    match cru {
        "true" | "True" => Valor::Booleano(true),
        "false" | "False" => Valor::Booleano(false),
        _ => match cru.parse::<f32>() {
            Ok(numero) if !cru.is_empty() && numero.is_finite() => Valor::Numero(numero),
            _ => Valor::Texto(cru.to_string()),
        },
    }
}

fn sem_extensao(nome: &str) -> String {
    let cortado = match nome.rfind('.') {
        Some(i) if i > 0 => &nome[..i],
        _ => nome,
    };
    let limpo = cortado.trim();
    if limpo.is_empty() {
        nome.to_string()
    } else {
        limpo.to_string()
    }
}

/// Um arquivo que o fotógrafo escolheu, já lido do disco.
///
/// `texto` é `None` quando a leitura falhou — permissão, arquivo binário, disco
/// removido no meio. **Não é a mesma coisa que "não é preset do Lightroom"**, e
/// o relatório separa as duas: uma se resolve escolhendo outro arquivo, a outra
/// não se resolve.
#[derive(Debug, Clone, PartialEq)]
pub struct Arquivo {
    pub nome: String,
    pub texto: Option<String>,
}

/// Quem abre o seletor do sistema e devolve o conteúdo lido.
///
/// 🔑 **Porta própria, como o [`super::presets::GuardaDePresets`]** e pelas
/// mesmas duas razões: abrir diálogo do sistema é `async` do tokio, e os testes
/// de tela precisam importar sem que nenhuma janela apareça na máquina de quem
/// roda a suíte.
///
/// ⚠️ **Responde sempre**, mesmo que com a lista vazia. Silêncio deixaria a tela
/// esperando arquivos que nunca vêm, com o botão desligado até fechar o app.
pub trait EscolhaDePresets: Send + Sync + 'static {
    fn escolher(&self, canal: std::sync::mpsc::Sender<Vec<Arquivo>>);
}

/// O seletor do sistema, via `rfd` — a mesma escolha da importação de fotos.
pub struct EscolhaNativa {
    tokio: tokio::runtime::Handle,
}

impl EscolhaNativa {
    pub fn nova(tokio: tokio::runtime::Handle) -> Self {
        Self { tokio }
    }
}

impl EscolhaDePresets for EscolhaNativa {
    fn escolher(&self, canal: std::sync::mpsc::Sender<Vec<Arquivo>>) {
        self.tokio.spawn(async move {
            let escolhidos = rfd::AsyncFileDialog::new()
                .set_title("Importar do Lightroom ou do darktable")
                .add_filter(
                    "Predefinições do Lightroom e estilos do darktable",
                    &["lrtemplate", "xmp", "dtstyle"],
                )
                .pick_files()
                .await
                .unwrap_or_default();

            let mut arquivos = Vec::with_capacity(escolhidos.len());
            for escolhido in escolhidos {
                let caminho = escolhido.path().to_path_buf();
                let nome = caminho
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| caminho.to_string_lossy().to_string());
                // ⚠️ **`read_to_string` recusa o que não é UTF-8**, e é o que se
                // quer: os dois formatos são texto, e um binário lido como
                // preset entraria como "sem nenhum ajuste" em vez de "não deu
                // para ler".
                arquivos.push(Arquivo {
                    nome,
                    texto: tokio::fs::read_to_string(&caminho).await.ok(),
                });
            }

            let _ = canal.send(arquivos);
        });
    }
}

/// O que a importação aproveitou — e o que não.
///
/// 🔑 **Sem isto, um preset de filme entraria como "importado com sucesso" e
/// mudaria menos do que o nome promete.** Dizer o que ficou de fora é a
/// diferença entre uma tradução e um engano silencioso: quem não vê o aviso
/// procura defeito no próprio olho, no monitor, ou no arquivo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Relatorio {
    pub arquivos: usize,
    pub criadas: usize,
    /// Já existia uma com o mesmo nome, e ela não foi tocada.
    pub repetidas: usize,
    /// Lidas, e sem nenhum ajuste que este motor aplique.
    pub sem_ajuste: Vec<String>,
    /// Nenhum dos leitores reconheceu o conteúdo.
    pub ilegiveis: Vec<String>,
    /// Recurso que este motor não tem, e em quantos arquivos apareceu — o que
    /// apareceu em mais arquivos primeiro.
    pub ignorados: Vec<(String, usize)>,
}

/// Lê os arquivos escolhidos e diz o que fazer com cada um.
///
/// Devolve as predefinições a criar (na ordem em que os arquivos vieram) e o
/// relatório do que ficou pelo caminho. **Não grava nada** — quem grava é a
/// tela, que é quem conhece a porta e a lista.
///
/// ⚠️ **Nome repetido é pulado, e não sobrescrito.** Reimportar a mesma pasta é
/// gesto comum, e sobrescrever apagaria o ajuste que o fotógrafo fez em cima da
/// predefinição depois de importá-la.
pub fn preparar(arquivos: &[Arquivo], ja_existem: &[String]) -> (Vec<PresetTraduzido>, Relatorio) {
    let mut relatorio = Relatorio {
        arquivos: arquivos.len(),
        ..Default::default()
    };
    let mut nomes: Vec<String> = ja_existem.to_vec();
    let mut criar = Vec::new();
    // Na ordem em que apareceram, como o `Map` do site: o desempate da
    // ordenação por contagem é esse.
    let mut contagem: Vec<(String, usize)> = Vec::new();

    for arquivo in arquivos {
        let Some(texto) = arquivo.texto.as_deref() else {
            relatorio.ilegiveis.push(arquivo.nome.clone());
            continue;
        };
        // 🔑 **O darktable vem primeiro, e decidido pelo conteúdo**: o `.xmp`
        // dele e o do Lightroom têm a mesma extensão, e o leitor do Lightroom
        // leria o do darktable como um preset sem ajuste nenhum.
        let traduzido = if darktable::eh_do_darktable(texto, &arquivo.nome) {
            darktable::ler(texto, &arquivo.nome)
        } else {
            ler_arquivo(texto, &arquivo.nome).map(|bruto| traduzir(&bruto))
        };
        let Some(traduzido) = traduzido else {
            relatorio.ilegiveis.push(arquivo.nome.clone());
            continue;
        };

        for rotulo in &traduzido.ignorados {
            match contagem.iter_mut().find(|(r, _)| r == rotulo) {
                Some((_, quantos)) => *quantos += 1,
                None => contagem.push((rotulo.clone(), 1)),
            }
        }

        if traduzido.ajustes.is_empty() {
            relatorio.sem_ajuste.push(traduzido.nome);
            continue;
        }
        if nomes.iter().any(|nome| nome == &traduzido.nome) {
            relatorio.repetidas += 1;
            continue;
        }

        nomes.push(traduzido.nome.clone());
        criar.push(traduzido);
    }

    relatorio.criadas = criar.len();
    relatorio.ignorados = contagem;
    // O que apareceu em mais arquivos primeiro: é o que mais falta ao motor.
    relatorio
        .ignorados
        .sort_by_key(|(_, quantos)| std::cmp::Reverse(*quantos));
    (criar, relatorio)
}

/// Uma linha por vez, para a tela desenhar o resultado.
impl Relatorio {
    pub fn linhas(&self) -> Vec<String> {
        let mut linhas = Vec::new();
        if self.repetidas > 0 {
            linhas.push(format!(
                "{} já existiam com o mesmo nome e foram puladas.",
                self.repetidas
            ));
        }
        if !self.sem_ajuste.is_empty() {
            linhas.push(format!(
                "{} não tinham nenhum ajuste que este motor aplica e ficaram de fora{}.",
                self.sem_ajuste.len(),
                if self.sem_ajuste.len() <= 3 {
                    format!(" ({})", self.sem_ajuste.join(", "))
                } else {
                    String::new()
                }
            ));
        }
        if !self.ilegiveis.is_empty() {
            linhas.push(format!("{} não puderam ser lidos.", self.ilegiveis.len()));
        }
        linhas
    }

    /// Os recursos que ficaram de fora, um por linha — `"3× remoção de névoa"`.
    pub fn linhas_dos_ignorados(&self) -> Vec<String> {
        self.ignorados
            .iter()
            .map(|(rotulo, quantos)| format!("{quantos}× {rotulo}"))
            .collect()
    }

    /// O cabeçalho: quantos arquivos entraram e quantas predefinições saíram.
    pub fn resumo(&self) -> String {
        format!(
            "{} {}: {} {}.",
            self.arquivos,
            if self.arquivos == 1 {
                "arquivo lido"
            } else {
                "arquivos lidos"
            },
            self.criadas,
            if self.criadas == 1 {
                "predefinição criada"
            } else {
                "predefinições criadas"
            }
        )
    }
}

/// A porta de mentira: responde com o que o teste mandar, sem abrir janela.
#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::{Arquivo, EscolhaDePresets};

    #[derive(Default)]
    pub struct EscolhaDeMentira {
        resposta: Mutex<Vec<Arquivo>>,
    }

    impl EscolhaDeMentira {
        pub fn com(arquivos: Vec<Arquivo>) -> Self {
            Self {
                resposta: Mutex::new(arquivos),
            }
        }
    }

    impl EscolhaDePresets for EscolhaDeMentira {
        fn escolher(&self, canal: std::sync::mpsc::Sender<Vec<Arquivo>>) {
            let resposta = self.resposta.lock().expect("a resposta do seletor").clone();
            let _ = canal.send(resposta);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Um `.lrtemplate` como o Lightroom os grava — a estrutura é Lua, não JSON.
    const LRTEMPLATE: &str = r#"s = {
	id = "D728662A-4FF5-4327-A6AA-444291159768",
	internalName = "Cross Process Cyan",
	title = "Cross Process-Cyan",
	type = "Develop",
	value = {
		settings = {
			Clarity2012 = 25,
			ConvertToGrayscale = false,
			Contrast2012 = -20,
			ParametricDarks = 0,
			ParametricHighlights = 12,
			ProcessVersion = "6.7",
			SaturationAdjustmentRed = -30,
			HueAdjustmentBlue = 100,
			LuminanceAdjustmentAqua = 40,
			Sharpness = 150,
			SharpenRadius = 1.4,
			ToneCurveName2012 = "Custom",
			ToneCurvePV2012Blue = {
				0,
				22,
				254,
				202,
			},
			SplitToningShadowHue = 210,
			SplitToningShadowSaturation = 30,
			GrainAmount = 25,
			Vibrance = -11,
		},
		uuid = "7F1F3C64-AE13-4B08-8715-04F23D50B4FE",
	},
	version = 0,
}"#;

    fn traduzir_chaves(pares: &[(&str, Valor)]) -> PresetTraduzido {
        traduzir(&PresetBruto {
            nome: "x".into(),
            ajustes: pares
                .iter()
                .map(|(chave, valor)| (chave.to_string(), valor.clone()))
                .collect(),
        })
    }

    fn n(v: f32) -> Valor {
        Valor::Numero(v)
    }

    #[test]
    fn le_o_nome_e_os_pares_do_bloco_settings() {
        let bruto = ler_lrtemplate(LRTEMPLATE, "Cross Process-Cyan.lrtemplate")
            .expect("é um preset do Lightroom");

        assert_eq!(bruto.nome, "Cross Process-Cyan");
        assert_eq!(bruto.ajustes.get("Clarity2012"), Some(&n(25.0)));
        assert_eq!(bruto.ajustes.get("Contrast2012"), Some(&n(-20.0)));
        assert_eq!(
            bruto.ajustes.get("ConvertToGrayscale"),
            Some(&Valor::Booleano(false))
        );
        assert_eq!(
            bruto.ajustes.get("ProcessVersion"),
            Some(&Valor::Texto("6.7".into()))
        );
        assert_eq!(bruto.ajustes.get("SharpenRadius"), Some(&n(1.4)));
    }

    /// ⚠️ A tabela aninhada (a curva) tem de ser **pulada inteira**: um leitor
    /// ingênuo pegaria os números dela como se fossem pares soltos e inventaria
    /// ajustes que o preset não tem.
    #[test]
    fn pula_a_tabela_aninhada_sem_confundir_os_numeros_dela_com_ajustes() {
        let bruto = ler_lrtemplate(LRTEMPLATE, "x.lrtemplate").expect("é um preset");

        assert_eq!(
            bruto.ajustes.get("ToneCurvePV2012Blue"),
            Some(&Valor::Tabela(vec![
                "0".into(),
                "22".into(),
                "254".into(),
                "202".into()
            ]))
        );
        assert_eq!(bruto.ajustes.get("SplitToningShadowHue"), Some(&n(210.0)));
        assert_eq!(bruto.ajustes.get("Vibrance"), Some(&n(-11.0)));
    }

    #[test]
    fn sem_o_bloco_settings_nao_e_um_preset() {
        assert!(ler_lrtemplate(r#"s = { id = "x" }"#, "x.lrtemplate").is_none());
    }

    #[test]
    fn sem_titulo_usa_o_nome_do_arquivo() {
        let sem_titulo = "s = { value = { settings = { Exposure2012 = 1 } } }";
        assert_eq!(
            ler_lrtemplate(sem_titulo, "Meu Preset.lrtemplate")
                .expect("é um preset")
                .nome,
            "Meu Preset"
        );
    }

    /// O `title` pode ser uma tabela de traduções — é como o Lightroom guarda
    /// preset localizado.
    #[test]
    fn aceita_titulo_em_tabela_de_traducoes() {
        let com_tabela =
            r#"s = { title = { z = "Filme Suave" }, value = { settings = { Exposure2012 = 1 } } }"#;
        assert_eq!(
            ler_lrtemplate(com_tabela, "x.lrtemplate")
                .expect("é um preset")
                .nome,
            "Filme Suave"
        );
    }

    #[test]
    fn o_xmp_le_valores_em_atributo() {
        let xmp = r#"<x:xmpmeta xmlns:crs="http://ns.adobe.com/camera-raw-settings/1.0/">
          <rdf:Description crs:PresetName="Retrato Claro" crs:Exposure2012="+0.35" crs:Contrast2012="15"/>
        </x:xmpmeta>"#;
        let bruto = ler_xmp(xmp, "arquivo.xmp").expect("é um preset");

        assert_eq!(bruto.nome, "Retrato Claro");
        assert_eq!(bruto.ajustes.get("Exposure2012"), Some(&n(0.35)));
        assert_eq!(bruto.ajustes.get("Contrast2012"), Some(&n(15.0)));
    }

    #[test]
    fn o_xmp_le_valores_em_elemento_e_o_nome_em_rdf_li() {
        let xmp = r#"<x:xmpmeta xmlns:crs="ns">
          <crs:PresetName><rdf:Alt><rdf:li xml:lang="x-default">Filme B</rdf:li></rdf:Alt></crs:PresetName>
          <crs:Exposure2012>-0.5</crs:Exposure2012>
          <crs:ConvertToGrayscale>True</crs:ConvertToGrayscale>
        </x:xmpmeta>"#;
        let bruto = ler_xmp(xmp, "outro.xmp").expect("é um preset");

        assert_eq!(bruto.nome, "Filme B");
        assert_eq!(bruto.ajustes.get("Exposure2012"), Some(&n(-0.5)));
        assert_eq!(
            bruto.ajustes.get("ConvertToGrayscale"),
            Some(&Valor::Booleano(true))
        );
    }

    #[test]
    fn um_xml_sem_crs_nao_e_preset_do_lightroom() {
        assert!(ler_xmp("<x><y>1</y></x>", "x.xmp").is_none());
    }

    #[test]
    fn escolhe_pela_extensao_e_cai_para_o_conteudo_quando_ela_mente() {
        assert_eq!(
            ler_arquivo(LRTEMPLATE, "p.lrtemplate")
                .expect("é um preset")
                .nome,
            "Cross Process-Cyan"
        );
        let xmp = r#"<x crs:PresetName="X" crs:Exposure2012="1"/>"#;
        assert_eq!(ler_arquivo(xmp, "p.xmp").expect("é um preset").nome, "X");
        // Extensão desconhecida: tenta os dois.
        assert_eq!(ler_arquivo(xmp, "p.txt").expect("é um preset").nome, "X");
        assert!(ler_arquivo("nada disso", "p.txt").is_none());
    }

    /// 🚨 **O contraste é multiplicador, e não porcentagem.**
    ///
    /// `Contrast2012 = 50` copiado sem converter vira `(v-128)*50+128` no
    /// shader: a foto fica em preto e branco puro, sem meio-tom. É o defeito
    /// que os presets de sistema tiveram por meses.
    #[test]
    fn o_contraste_vira_multiplicador_com_neutro_em_um() {
        assert_eq!(
            traduzir_chaves(&[("Contrast2012", n(-20.0))])
                .ajustes
                .get("contrast"),
            Some(0.8)
        );
        assert_eq!(
            traduzir_chaves(&[("Contrast2012", n(100.0))])
                .ajustes
                .get("contrast"),
            Some(2.0)
        );
        assert_eq!(
            traduzir_chaves(&[("Contrast2012", n(0.0))])
                .ajustes
                .get("contrast"),
            None,
            "o neutro não entra"
        );
    }

    /// Saturação, intensidade e textura vão de -100..100 para -1..1.
    #[test]
    fn as_tres_de_menos_um_a_um() {
        let traduzido = traduzir_chaves(&[
            ("Saturation", n(-100.0)),
            ("Vibrance", n(25.0)),
            ("Clarity2012", n(25.0)),
        ]);

        assert_eq!(traduzido.ajustes.get("saturation"), Some(-1.0));
        assert_eq!(traduzido.ajustes.get("vibrance"), Some(0.25));
        assert_eq!(traduzido.ajustes.get("clarity"), Some(0.25));
    }

    /// 🔑 O HSL casa cor a cor, e o matiz vira **graus**.
    ///
    /// O extremo do slider da Adobe desloca cerca de 30°, não meia volta:
    /// mapear -100..100 em -180..180 giraria a cor muito além do que o preset
    /// pede — e o rosto ficaria verde num preset que só esquentava a pele.
    #[test]
    fn o_hsl_casa_cor_a_cor_e_o_matiz_vira_graus() {
        let traduzido = traduzir_chaves(&[
            ("SaturationAdjustmentRed", n(-30.0)),
            ("LuminanceAdjustmentAqua", n(40.0)),
            ("HueAdjustmentBlue", n(100.0)),
        ]);

        assert_eq!(traduzido.ajustes.get("hsl_red_sat"), Some(-30.0));
        assert_eq!(traduzido.ajustes.get("hsl_aqua_lum"), Some(40.0));
        assert_eq!(traduzido.ajustes.get("hsl_blue_hue"), Some(30.0));
    }

    /// A nitidez da Adobe vai a 150; a daqui, a 100.
    #[test]
    fn a_nitidez_de_zero_a_cento_e_cinquenta_vira_de_zero_a_cem() {
        let traduzido = traduzir_chaves(&[("Sharpness", n(150.0)), ("SharpenRadius", n(1.4))]);

        assert_eq!(traduzido.ajustes.get("sharpen_amount"), Some(100.0));
        assert_eq!(traduzido.ajustes.get("sharpen_radius"), Some(1.4));
    }

    /// ⚠️ A vinheta que o preset usa como efeito é a **pós-corte**; a de lente
    /// é correção óptica e some quando há corte.
    #[test]
    fn a_vinheta_e_a_pos_corte() {
        let traduzido = traduzir_chaves(&[
            ("PostCropVignetteAmount", n(-40.0)),
            ("VignetteAmount", n(-40.0)),
        ]);

        assert_eq!(traduzido.ajustes.get("lens_vignette_amount"), Some(-40.0));
        assert_eq!(traduzido.ajustes.len(), 1);
    }

    /// 🚨 **Um preset P&B acende o mixer, e não só dessatura.** Só a
    /// dessaturação dava cinza chapado, e os oito `GrayMixer*` do mesmo arquivo
    /// não tinham onde agir.
    #[test]
    fn o_preto_e_branco_acende_o_mixer_e_traz_a_mistura_por_canal() {
        let traduzido = traduzir_chaves(&[
            ("ConvertToGrayscale", Valor::Booleano(true)),
            ("GrayMixerRed", n(40.0)),
            ("GrayMixerBlue", n(-60.0)),
        ]);
        assert_eq!(traduzido.ajustes.get("bw_ativo"), Some(1.0));
        assert_eq!(traduzido.ajustes.get("saturation"), Some(-1.0));
        assert_eq!(traduzido.ajustes.get("bw_red"), Some(40.0));
        assert_eq!(traduzido.ajustes.get("bw_blue"), Some(-60.0));
        assert!(
            traduzir_chaves(&[("ConvertToGrayscale", Valor::Booleano(false))])
                .ajustes
                .is_empty()
        );
    }

    /// ✅ **A calibração de câmera atravessa** — a base da maioria dos presets
    /// de filme, contada como "ignorada" até aqui.
    #[test]
    fn a_calibracao_de_camera_vira_os_sete_controles_de_calibracao() {
        let traduzido = traduzir_chaves(&[
            ("RedHue", n(12.0)),
            ("RedSaturation", n(-8.0)),
            ("GreenHue", n(-20.0)),
            ("GreenSaturation", n(15.0)),
            ("BlueHue", n(30.0)),
            ("BlueSaturation", n(-25.0)),
            ("ShadowTint", n(6.0)),
        ]);
        let esperado: PresetAdjustments = [
            ("calib_red_hue", 12.0),
            ("calib_red_sat", -8.0),
            ("calib_green_hue", -20.0),
            ("calib_green_sat", 15.0),
            ("calib_blue_hue", 30.0),
            ("calib_blue_sat", -25.0),
            ("calib_shadow_tint", 6.0),
        ]
        .into_iter()
        .collect();
        assert_eq!(traduzido.ajustes, esperado);
        assert!(traduzido.ignorados.is_empty());
    }

    #[test]
    fn contraste_nos_extremos_e_no_meio() {
        let contraste = |v| {
            traduzir_chaves(&[("Contrast2012", n(v))])
                .ajustes
                .get("contrast")
        };
        assert_eq!(contraste(-100.0), Some(0.0));
        assert_eq!(contraste(25.0), Some(1.25));
        assert_eq!(contraste(50.0), Some(1.5));
    }

    #[test]
    fn a_exposicao_ja_vem_na_mesma_unidade_e_as_zonas_passam_direto() {
        let traduzido = traduzir_chaves(&[
            ("Exposure2012", n(1.35)),
            ("Highlights2012", n(-60.0)),
            ("Shadows2012", n(40.0)),
            ("Whites2012", n(10.0)),
            ("Blacks2012", n(-15.0)),
            ("ParametricLights", n(22.0)),
        ]);
        assert_eq!(traduzido.ajustes.get("exposure"), Some(1.35));
        assert_eq!(traduzido.ajustes.get("highlights"), Some(-60.0));
        assert_eq!(traduzido.ajustes.get("shadows"), Some(40.0));
        assert_eq!(traduzido.ajustes.get("whites"), Some(10.0));
        assert_eq!(traduzido.ajustes.get("blacks"), Some(-15.0));
        assert_eq!(traduzido.ajustes.get("tone_curve_lights"), Some(22.0));
        assert_eq!(
            traduzir_chaves(&[("Exposure2012", n(99.0))])
                .ajustes
                .get("exposure"),
            Some(5.0)
        );
    }

    /// Um campo no neutro não é ajuste — inclusive o raio de nitidez, cujo
    /// neutro é 1 e não zero.
    #[test]
    fn valor_neutro_nao_entra_na_predefinicao() {
        let traduzido = traduzir_chaves(&[
            ("Exposure2012", n(0.0)),
            ("Saturation", n(0.0)),
            ("SharpenRadius", n(1.0)),
            ("ColorGradeBlending", n(50.0)),
        ]);
        assert!(traduzido.ajustes.is_empty());
    }

    /// 🚨 **A curva por ponto chega ao motor**, e era o maior buraco: 321 dos
    /// 400 presets da amostra a usavam.
    #[test]
    fn a_curva_por_ponto_vira_as_nove_alturas_do_canal() {
        // O formato do XMP: um ponto por item, "x, y".
        let traduzido = traduzir_chaves(&[(
            "ToneCurvePV2012",
            Valor::Tabela(vec!["0, 20".into(), "128, 128".into(), "255, 235".into()]),
        )]);
        assert_eq!(traduzido.ajustes.get("curva_m0"), Some(20.0));
        assert_eq!(traduzido.ajustes.get("curva_m8"), Some(235.0));
        assert_eq!(traduzido.ajustes.len(), 9);
        assert!(traduzido.ignorados.is_empty());

        // A curva intocada, que o Lightroom grava mesmo assim, não entra.
        let neutra = traduzir_chaves(&[(
            "ToneCurvePV2012",
            Valor::Tabela(vec!["0, 0".into(), "255, 255".into()]),
        )]);
        assert!(neutra.ajustes.is_empty());

        // A nova manda; a antiga só vale sem a nova.
        let duas = traduzir_chaves(&[
            (
                "ToneCurvePV2012Red",
                Valor::Tabela(vec!["0, 30".into(), "255, 255".into()]),
            ),
            (
                "ToneCurveRed",
                Valor::Tabela(vec!["0, 90".into(), "255, 255".into()]),
            ),
        ]);
        assert_eq!(duas.ajustes.get("curva_r0"), Some(30.0));

        // Menos de dois pontos não é curva.
        assert!(
            traduzir_chaves(&[("ToneCurve", Valor::Tabela(vec!["0, 40".into()]))])
                .ajustes
                .is_empty()
        );
    }

    /// Os quatro eixos do Color Grading e a mistura.
    #[test]
    fn os_quatro_eixos_do_color_grading_viram_tonalizacao() {
        let traduzido = traduzir_chaves(&[
            ("ColorGradeShadowHue", n(210.0)),
            ("ColorGradeShadowSat", n(30.0)),
            ("ColorGradeMidtoneHue", n(120.0)),
            ("ColorGradeMidtoneSat", n(20.0)),
            ("ColorGradeHighlightHue", n(45.0)),
            ("ColorGradeHighlightSat", n(15.0)),
            ("ColorGradeGlobalHue", n(30.0)),
            ("ColorGradeGlobalSat", n(10.0)),
            ("ColorGradeBlending", n(80.0)),
        ]);
        let esperado: PresetAdjustments = [
            ("split_shadow_hue", 210.0),
            ("split_shadow_sat", 30.0),
            ("split_midtone_hue", 120.0),
            ("split_midtone_sat", 20.0),
            ("split_highlight_hue", 45.0),
            ("split_highlight_sat", 15.0),
            ("split_global_hue", 30.0),
            ("split_global_sat", 10.0),
            ("split_blending", 80.0),
        ]
        .into_iter()
        .collect();
        assert_eq!(traduzido.ajustes, esperado);
        assert!(traduzido.ignorados.is_empty());
    }

    /// 🚨 **Valor que não é número é ignorado sem quebrar**, e valor fora da
    /// faixa é recortado: um preset com matiz 400 é arquivo estragado, não
    /// escala diferente.
    #[test]
    fn valor_estranho_nao_quebra_e_valor_fora_da_faixa_e_recortado() {
        let traduzido = traduzir_chaves(&[
            ("Exposure2012", Valor::Texto("muito".into())),
            ("SplitToningShadowHue", n(400.0)),
            ("Saturation", n(-500.0)),
        ]);

        assert_eq!(traduzido.ajustes.get("exposure"), None);
        assert_eq!(traduzido.ajustes.get("split_shadow_hue"), Some(360.0));
        assert_eq!(traduzido.ajustes.get("saturation"), Some(-1.0));
    }

    #[test]
    fn conta_o_que_ficou_de_fora_por_rotulo() {
        let traduzido = traduzir_chaves(&[
            (
                "ToneCurvePV2012Blue",
                Valor::Tabela(vec!["0".into(), "22".into()]),
            ),
            ("ColorGradeMidtoneHue", n(120.0)),
            ("GrainFrequency", n(40.0)),
            ("Dehaze", n(10.0)),
            ("DehazeExtra", n(3.0)),
            ("RedHue", n(5.0)),
            ("Contrast2012", n(20.0)),
        ]);

        // ✅ Curva por ponto, tons médios e calibração não são mais "ignorados":
        // agem. E o mesmo rótulo não se repete por duas chaves da família.
        assert_eq!(
            traduzido.ignorados,
            vec!["aspereza do grão", "remoção de névoa"]
        );
    }

    /// 🚨 **`GrainAmount` começa com "Grain", e `GrainFrequency` também.**
    ///
    /// Enquanto o motor não tinha grão, um prefixo "Grain" na lista de
    /// ignorados dava conta dos dois. Agora um deles é traduzido e o outro não
    /// — e um prefixo largo demais faria o preset avisar "ignorei o grão" logo
    /// depois de tê-lo aplicado. O mesmo vale para "SplitToning" e "ColorGrade".
    #[test]
    fn o_que_o_motor_ganhou_saiu_da_lista_de_ignorados() {
        let traduzido = traduzir_chaves(&[
            ("SplitToningShadowHue", n(35.0)),
            ("SplitToningShadowSaturation", n(40.0)),
            ("SplitToningHighlightHue", n(50.0)),
            ("SplitToningHighlightSaturation", n(20.0)),
            ("SplitToningBalance", n(-25.0)),
            ("GrainAmount", n(25.0)),
            ("GrainSize", n(30.0)),
        ]);

        assert_eq!(traduzido.ajustes.len(), 7);
        assert_eq!(traduzido.ajustes.get("split_shadow_hue"), Some(35.0));
        assert_eq!(traduzido.ajustes.get("grain_amount"), Some(25.0));
        assert!(traduzido.ignorados.is_empty());
    }

    /// Zero e `false` não mudam nada — avisar sobre eles seria ruído.
    #[test]
    fn o_que_esta_desligado_nao_conta_como_ignorado() {
        let traduzido = traduzir_chaves(&[
            ("ColorGradeMidtoneSat", n(0.0)),
            ("GrainFrequency", n(0.0)),
            ("Dehaze", n(0.0)),
            ("ToneCurveName2012", Valor::Texto("Linear".into())),
            ("Contrast2012", n(10.0)),
        ]);

        assert!(traduzido.ignorados.is_empty());
    }

    /// O caminho inteiro, num preset de verdade: lê, traduz o que dá, e avisa o
    /// resto.
    #[test]
    fn o_preset_real_traduz_o_que_da_e_avisa_o_resto() {
        let bruto = ler_lrtemplate(LRTEMPLATE, "Cross Process-Cyan.lrtemplate")
            .expect("é um preset do Lightroom");
        let traduzido = traduzir(&bruto);

        assert_eq!(traduzido.nome, "Cross Process-Cyan");
        assert_eq!(traduzido.ajustes.get("clarity"), Some(0.25));
        assert_eq!(traduzido.ajustes.get("contrast"), Some(0.8));
        assert_eq!(traduzido.ajustes.get("vibrance"), Some(-0.11));
        assert_eq!(traduzido.ajustes.get("hsl_red_sat"), Some(-30.0));
        assert_eq!(traduzido.ajustes.get("sharpen_amount"), Some(100.0));
        // ✅ O que era "split toning" e "grão" na lista de ignorados agora é
        // ajuste.
        assert_eq!(traduzido.ajustes.get("split_shadow_hue"), Some(210.0));
        assert_eq!(traduzido.ajustes.get("split_shadow_sat"), Some(30.0));
        assert_eq!(traduzido.ajustes.get("grain_amount"), Some(25.0));
        // ✅ **E a curva por ponto chega**: a deste preset é só no azul, e é
        // dela que sai o "cross process" — preto azulado levantado, alta-luz
        // contida. O canal mestre fica na reta.
        assert!(traduzido.ignorados.is_empty());
        assert!(traduzido.ajustes.get("curva_b0").expect("curva azul") > 20.0);
        assert!(traduzido.ajustes.get("curva_b8").expect("curva azul") < 220.0);
        assert_eq!(traduzido.ajustes.get("curva_m0"), None);
    }

    /// 🔑 **Todo campo que a tradução escreve existe no motor.**
    ///
    /// A tabela de conversões é de `&'static str`, e um erro de digitação em
    /// `"split_shadow_hue"` daria um preset que importa "com sucesso" e não
    /// move nada — sem erro em lugar nenhum.
    #[test]
    fn nenhuma_conversao_aponta_para_campo_inventado() {
        let chaves = [
            "Exposure2012",
            "Exposure",
            "Contrast2012",
            "Contrast",
            "Highlights2012",
            "Shadows2012",
            "Whites2012",
            "Blacks2012",
            "Clarity2012",
            "Clarity",
            "Vibrance",
            "Saturation",
            "ParametricShadows",
            "ParametricDarks",
            "ParametricLights",
            "ParametricHighlights",
            "LuminanceSmoothing",
            "ColorNoiseReduction",
            "Sharpness",
            "SharpenRadius",
            "PostCropVignetteAmount",
            "PostCropVignetteMidpoint",
            "LensManualDistortionAmount",
            "SplitToningShadowHue",
            "SplitToningShadowSaturation",
            "SplitToningHighlightHue",
            "SplitToningHighlightSaturation",
            "SplitToningBalance",
            "ColorGradeShadowHue",
            "ColorGradeShadowSat",
            "ColorGradeHighlightHue",
            "ColorGradeHighlightSat",
            "ColorGradeMidtoneHue",
            "ColorGradeMidtoneSat",
            "ColorGradeGlobalHue",
            "ColorGradeGlobalSat",
            "ColorGradeBlending",
            "RedHue",
            "RedSaturation",
            "GreenHue",
            "GreenSaturation",
            "BlueHue",
            "BlueSaturation",
            "ShadowTint",
            "GrainAmount",
            "GrainSize",
        ];

        for chave in chaves {
            let conversao = conversao(chave).unwrap_or_else(|| panic!("`{chave}` sumiu da tabela"));
            assert!(
                Ajustes::NOMES.contains(&conversao.campo),
                "`{chave}` escreve em `{}`, que não está em `Ajustes::NOMES`",
                conversao.campo
            );
        }

        for (deles, _) in CORES {
            for familia in [
                "SaturationAdjustment",
                "LuminanceAdjustment",
                "HueAdjustment",
                "GrayMixer",
            ] {
                let chave = format!("{familia}{deles}");
                let conversao =
                    conversao(&chave).unwrap_or_else(|| panic!("`{chave}` sumiu da tabela"));
                assert!(Ajustes::NOMES.contains(&conversao.campo), "`{chave}`");
            }
        }
    }
}
