//! Ler estilos do darktable — o `.dtstyle` e o `.xmp` que ele grava ao lado da
//! foto. É o porte de `revelacao/darktable.ts` do site.
//!
//! # Por que não é uma tabela de conversão, como a do Lightroom
//!
//! 🔑 **O darktable não guarda números soltos: guarda a struct de cada módulo em
//! binário**, na versão daquele módulo (`op_params`, em hexadecimal ou `gz` +
//! base64 + zlib). E o motor não traduz esses números para os controles do
//! Lightroom — ele tem os próprios módulos do darktable portados
//! (`revelacao-core/src/darktable.rs`), com os mesmos campos. Então importar é
//! ler a struct e copiar campo a campo, sem escala nenhuma no meio.
//!
//! # O que ele se recusa a importar calado
//!
//! 🚨 **Um estilo que o motor não reproduz não pode entrar como "importado".**
//! Cada motivo vira uma linha no relatório da coluna, e o módulo afetado fica de
//! fora em vez de entrar diferente:
//!
//! - módulo que ainda não foi portado;
//! - versão da struct que não é a do darktable 5.6.1 — a mesma posição de byte
//!   quer dizer outro campo em outra versão;
//! - mistura que não é normal a 100 % sem máscara;
//! - segunda instância do mesmo módulo, que o motor não tem onde pôr.
//!
//! Os módulos de tubulação (`colorin`, `colorout`, `gamma`, `flip`) não viram
//! ajuste: o motor já faz sRGB → Rec.2020 linear → sRGB, que é o padrão do
//! darktable para JPEG. Só avisam quando o estilo pede outra coisa.
//!
//! # ⚠️ Sem `regex`, como o leitor do Lightroom
//!
//! Os padrões do site cabem em `find` — `<plugin>…</plugin>`, `<rdf:li …/>`,
//! `darktable:nome="…"`.

use domain::entities::preset::PresetAdjustments;

use super::PresetTraduzido;
use crate::revelacao::processador::Ajustes;

/// Uma entrada do histórico: um `<plugin>` do `.dtstyle` ou um `<rdf:li>` do
/// `.xmp`.
#[derive(Debug, Clone)]
struct Entrada {
    num: f64,
    operacao: String,
    versao: f64,
    ativo: bool,
    params: String,
    blend: String,
    prioridade: f64,
}

/// O que um módulo portado leu: os campos, e o que entrou mas não inteiro.
struct Leitura {
    ajustes: Vec<(&'static str, f32)>,
    avisos: Vec<String>,
}

/// Os bytes de uma struct, lidos em little-endian.
struct Bytes<'a>(&'a [u8]);

impl Bytes<'_> {
    fn quatro(&self, deslocamento: usize) -> [u8; 4] {
        let mut b = [0u8; 4];
        b.copy_from_slice(&self.0[deslocamento..deslocamento + 4]);
        b
    }
    fn f32(&self, deslocamento: usize) -> f32 {
        f32::from_le_bytes(self.quatro(deslocamento))
    }
    fn i32(&self, deslocamento: usize) -> i32 {
        i32::from_le_bytes(self.quatro(deslocamento))
    }
    fn u32(&self, deslocamento: usize) -> u32 {
        u32::from_le_bytes(self.quatro(deslocamento))
    }
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Um módulo portado: a versão da struct que se sabe ler, o tamanho mínimo e a
/// leitura — que devolve os campos ou o motivo de não importar o módulo.
struct Modulo {
    versao: f64,
    tamanho: usize,
    ler: fn(&Bytes) -> Result<Leitura, String>,
}

/// A ordem dos 32 `float` do `dt_iop_colorbalancergb_params_t` v5, já com o
/// nome do campo do motor (`dt_cb_` + o do darktable em minúsculas).
const CAMPOS_DO_COLOR_BALANCE: [&str; 32] = [
    "dt_cb_shadows_y",
    "dt_cb_shadows_c",
    "dt_cb_shadows_h",
    "dt_cb_midtones_y",
    "dt_cb_midtones_c",
    "dt_cb_midtones_h",
    "dt_cb_highlights_y",
    "dt_cb_highlights_c",
    "dt_cb_highlights_h",
    "dt_cb_global_y",
    "dt_cb_global_c",
    "dt_cb_global_h",
    "dt_cb_shadows_weight",
    "dt_cb_white_fulcrum",
    "dt_cb_highlights_weight",
    "dt_cb_chroma_shadows",
    "dt_cb_chroma_highlights",
    "dt_cb_chroma_global",
    "dt_cb_chroma_midtones",
    "dt_cb_saturation_global",
    "dt_cb_saturation_highlights",
    "dt_cb_saturation_midtones",
    "dt_cb_saturation_shadows",
    "dt_cb_hue_angle",
    "dt_cb_brilliance_global",
    "dt_cb_brilliance_highlights",
    "dt_cb_brilliance_midtones",
    "dt_cb_brilliance_shadows",
    "dt_cb_mask_grey_fulcrum",
    "dt_cb_vibrance",
    "dt_cb_grey_fulcrum",
    "dt_cb_contrast",
];

/// `1.0` quando o inteiro não é zero — os `gboolean` das structs.
fn ligado(v: i32) -> f32 {
    if v != 0 {
        1.0
    } else {
        0.0
    }
}

/// Os módulos que o motor reproduz, com as structs do darktable 5.6.1
/// (`src/iop/*.c`, conferidas em `docs/DARKTABLE_CONFERENCIA.md` do site).
fn modulo(operacao: &str) -> Option<Modulo> {
    Some(match operacao {
        // mode, black, exposure, deflicker_percentile, deflicker_target_level,
        // compensate_exposure_bias, compensate_hilite_pres
        "exposure" => Modulo {
            versao: 7.0,
            tamanho: 28,
            ler: |v| {
                if v.i32(0) != 0 {
                    return Err(
                        "exposure no modo automático (deflicker), que só existe para RAW".into(),
                    );
                }
                let mut avisos = Vec::new();
                if v.i32(20) != 0 {
                    avisos.push("exposure: compensar o viés de exposição do EXIF".to_string());
                }
                Ok(Leitura {
                    ajustes: vec![
                        ("dt_exposure_ativo", 1.0),
                        ("dt_exposure_black", v.f32(4)),
                        ("dt_exposure_exposure", v.f32(8)),
                    ],
                    avisos,
                })
            },
        },
        // order, radius, shadows, whitepoint, highlights, reserved2, compress,
        // shadows_ccorrect, highlights_ccorrect, flags, low_approximation, shadhi_algo
        "shadhi" => Modulo {
            versao: 5.0,
            tamanho: 48,
            ler: |v| {
                if v.i32(44) != 1 {
                    return Err("shadows and highlights com suavização gaussiana (só a bilateral está portada)".into());
                }
                Ok(Leitura {
                    ajustes: vec![
                        ("dt_shadhi_ativo", 1.0),
                        ("dt_shadhi_radius", v.f32(4)),
                        ("dt_shadhi_shadows", v.f32(8)),
                        ("dt_shadhi_whitepoint", v.f32(12)),
                        ("dt_shadhi_highlights", v.f32(16)),
                        ("dt_shadhi_compress", v.f32(24)),
                        ("dt_shadhi_shadows_ccorrect", v.f32(28)),
                        ("dt_shadhi_highlights_ccorrect", v.f32(32)),
                        ("dt_shadhi_flags", v.u32(36) as f32),
                    ],
                    avisos: Vec::new(),
                })
            },
        },
        // a, b, size, highlights
        "monochrome" => Modulo {
            versao: 2.0,
            tamanho: 16,
            ler: |v| {
                Ok(Leitura {
                    ajustes: vec![
                        ("dt_monochrome_ativo", 1.0),
                        ("dt_monochrome_a", v.f32(0)),
                        ("dt_monochrome_b", v.f32(4)),
                        ("dt_monochrome_size", v.f32(8)),
                        ("dt_monochrome_highlights", v.f32(12)),
                    ],
                    avisos: Vec::new(),
                })
            },
        },
        // scale, falloff_scale, brightness, saturation, center.x, center.y,
        // autoratio, whratio, shape, dithering, unbound
        "vignette" => Modulo {
            versao: 4.0,
            tamanho: 44,
            ler: |v| {
                // O pontilhamento é ruído de ±1 nível contra degraus no
                // gradiente: a vinheta entra, e o relatório diz o que faltou.
                let mut avisos = Vec::new();
                if v.i32(36) != 0 {
                    avisos.push("vignetting: pontilhamento".to_string());
                }
                Ok(Leitura {
                    ajustes: vec![
                        ("dt_vignette_ativo", 1.0),
                        ("dt_vignette_scale", v.f32(0)),
                        ("dt_vignette_falloff_scale", v.f32(4)),
                        ("dt_vignette_brightness", v.f32(8)),
                        ("dt_vignette_saturation", v.f32(12)),
                        ("dt_vignette_center_x", v.f32(16)),
                        ("dt_vignette_center_y", v.f32(20)),
                        ("dt_vignette_autoratio", ligado(v.i32(24))),
                        ("dt_vignette_whratio", v.f32(28)),
                        ("dt_vignette_shape", v.f32(32)),
                        ("dt_vignette_unbound", ligado(v.i32(40))),
                    ],
                    avisos,
                })
            },
        },
        "colorbalancergb" => Modulo {
            versao: 5.0,
            tamanho: 132,
            ler: |v| {
                if v.i32(128) != 1 {
                    return Err("color balance rgb com a fórmula de saturação antiga (só a dt UCS está portada)".into());
                }
                let mut ajustes = vec![("dt_cb_ativo", 1.0)];
                for (i, campo) in CAMPOS_DO_COLOR_BALANCE.iter().enumerate() {
                    ajustes.push((*campo, v.f32(i * 4)));
                }
                Ok(Leitura {
                    ajustes,
                    avisos: Vec::new(),
                })
            },
        },
        _ => return None,
    })
}

/// Perfis de entrada que, numa foto JPEG, são o sRGB que o motor supõe:
/// SRGB, EMBEDDED_ICC e EMBEDDED_MATRIX.
const PERFIS_DE_ENTRADA_SRGB: [i32; 3] = [1, 9, 10];

/// A tubulação de cor e a orientação: não viram ajuste, só avisam quando o
/// estilo pede outra coisa que a do motor. `None` = não é tubulação.
fn tubulacao(operacao: &str) -> Option<fn(&Bytes) -> Option<String>> {
    Some(match operacao {
        // type, filename[512], intent, normalize, blue_mapping, type_work,
        // filename_work[512]
        "colorin" => |v| {
            if v.len() < 532 {
                return None;
            }
            if !PERFIS_DE_ENTRADA_SRGB.contains(&v.i32(0)) {
                return Some("colorin: perfil de entrada diferente do sRGB".into());
            }
            if v.i32(520) != 0 {
                return Some("colorin: recorte de gamut".into());
            }
            if v.i32(528) != 4 {
                return Some("colorin: espaço de trabalho diferente do Rec.2020 linear".into());
            }
            None
        },
        // type, filename[512], intent
        "colorout" => |v| {
            (v.len() >= 4 && v.i32(0) != 1)
                .then(|| "colorout: perfil de saída diferente do sRGB".into())
        },
        "gamma" => |_| None,
        "flip" => |v| {
            (v.len() >= 4 && v.i32(0) != -1)
                .then(|| "flip: orientação fixa no estilo (use o enquadramento)".into())
        },
        _ => return None,
    })
}

/// `gz` + dois dígitos + base64 de zlib, ou hexadecimal cru.
fn decodificar(valor: &str) -> Option<Vec<u8>> {
    if let Some(resto) = valor.strip_prefix("gz") {
        let comprimido = base64(resto.get(2..)?)?;
        return miniz_oxide::inflate::decompress_to_vec_zlib(&comprimido).ok();
    }
    if !valor.len().is_multiple_of(2) || !valor.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..valor.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&valor[i..i + 2], 16).ok())
        .collect()
}

/// Base64 padrão (o `atob` do site). Espaço em branco é ignorado; caractere
/// fora do alfabeto torna o valor ilegível.
fn base64(texto: &str) -> Option<Vec<u8>> {
    let valor = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let limpo: Vec<u8> = texto.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let sem_preenchimento = limpo
        .strip_suffix(b"==")
        .or_else(|| limpo.strip_suffix(b"="));
    let dados = sem_preenchimento.unwrap_or(&limpo);
    if dados.len() % 4 == 1 {
        return None;
    }
    let mut saida = Vec::with_capacity(dados.len() * 3 / 4);
    for bloco in dados.chunks(4) {
        let mut acumulado = 0u32;
        for (i, c) in bloco.iter().enumerate() {
            acumulado |= valor(*c)? << (18 - 6 * i);
        }
        let bytes = acumulado.to_be_bytes();
        saida.extend_from_slice(&bytes[1..bloco.len()]);
    }
    Some(saida)
}

/// A mistura do módulo é a que o motor aplica — normal, 100 %, sem máscara?
///
/// `dt_develop_blend_params_t` v14: `mask_mode` (u32), `blend_cst` (i32),
/// `blend_mode` (u32), `blend_parameter`, `opacity` (`src/develop/blend.h:180`).
/// `Err` = parâmetros ilegíveis.
fn motivo_da_mistura(blend: &str) -> Result<Option<&'static str>, ()> {
    if blend.is_empty() {
        return Ok(None);
    }
    let bytes = decodificar(blend).ok_or(())?;
    let v = Bytes(&bytes);
    if v.len() < 20 {
        return Ok(Some("mistura ilegível"));
    }
    let mascara = v.u32(0);
    // DEVELOP_MASK_DISABLED: o módulo vale inteiro, e o resto não é lido.
    if mascara == 0 {
        return Ok(None);
    }
    if mascara & !1 != 0 {
        return Ok(Some("máscara desenhada, paramétrica ou raster"));
    }
    let modo = v.u32(8);
    // NORMAL2 e os três "normais" antigos que o darktable converte nele.
    if modo & 0x8000_0000 != 0 || ![0x00, 0x01, 0x15, 0x18].contains(&(modo & 0xff)) {
        return Ok(Some("modo de mistura diferente do normal"));
    }
    if v.f32(16) < 100.0 {
        return Ok(Some("opacidade abaixo de 100 %"));
    }
    Ok(None)
}

/// O conteúdo do primeiro `<nome>…</nome>` do trecho, ou vazio.
fn elemento<'a>(bloco: &'a str, nome: &str) -> &'a str {
    let abre = format!("<{nome}>");
    let fecha = format!("</{nome}>");
    let Some(inicio) = bloco.find(&abre).map(|i| i + abre.len()) else {
        return "";
    };
    bloco[inicio..]
        .find(&fecha)
        .map_or("", |fim| &bloco[inicio..inicio + fim])
}

/// O valor de `darktable:nome="…"` no trecho, ou vazio.
fn atributo<'a>(li: &'a str, nome: &str) -> &'a str {
    let chave = format!("darktable:{nome}=\"");
    let Some(inicio) = li.find(&chave).map(|i| i + chave.len()) else {
        return "";
    };
    li[inicio..]
        .find('"')
        .map_or("", |fim| &li[inicio..inicio + fim])
}

/// O `Number(…)` do JavaScript para o que o darktable escreve: vazio é zero, e
/// o que não é número não é número.
fn numero(texto: &str) -> f64 {
    let texto = texto.trim();
    if texto.is_empty() {
        0.0
    } else {
        texto.parse().unwrap_or(f64::NAN)
    }
}

fn desescapar(texto: &str) -> String {
    texto
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn entradas_do_estilo(texto: &str) -> Vec<Entrada> {
    let mut saida = Vec::new();
    let mut resto = texto;
    while let Some(i) = resto.find("<plugin>") {
        let depois = &resto[i + "<plugin>".len()..];
        let Some(fim) = depois.find("</plugin>") else {
            break;
        };
        let p = &depois[..fim];
        saida.push(Entrada {
            num: numero(elemento(p, "num")),
            operacao: elemento(p, "operation").trim().to_string(),
            versao: numero(elemento(p, "module")),
            ativo: elemento(p, "enabled").trim() == "1",
            params: elemento(p, "op_params").trim().to_string(),
            blend: elemento(p, "blendop_params").trim().to_string(),
            prioridade: numero(elemento(p, "multi_priority")),
        });
        resto = &depois[fim..];
    }
    saida
}

/// O histórico do `.xmp` do darktable, **até o `history_end`**: o que está
/// depois dele foi desfeito na tela do darktable e não entra na foto.
fn entradas_do_xmp(texto: &str) -> Vec<Entrada> {
    let fim = texto
        .find("darktable:history_end=\"")
        .map(|i| &texto[i + "darktable:history_end=\"".len()..])
        .and_then(|resto| {
            let digitos: String = resto.chars().take_while(char::is_ascii_digit).collect();
            (resto[digitos.len()..].starts_with('"') && !digitos.is_empty())
                .then(|| digitos.parse::<f64>().ok())
                .flatten()
        })
        .unwrap_or(f64::INFINITY);
    let historico = elemento(texto, "darktable:history");

    let mut saida = Vec::new();
    let mut resto = historico;
    while let Some(i) = resto.find("<rdf:li") {
        let depois = &resto[i..];
        // `<rdf:li\b` — "rdf:list" não é item.
        let seguinte = depois[7..].chars().next();
        if seguinte.is_some_and(|c| c.is_alphanumeric() || c == '_') {
            resto = &depois[7..];
            continue;
        }
        let Some(fecha) = depois.find('>') else {
            break;
        };
        let li = &depois[..=fecha];
        let entrada = Entrada {
            num: numero(atributo(li, "num")),
            operacao: atributo(li, "operation").to_string(),
            versao: numero(atributo(li, "modversion")),
            ativo: atributo(li, "enabled") == "1",
            params: atributo(li, "params").to_string(),
            blend: atributo(li, "blendop_params").to_string(),
            prioridade: numero(atributo(li, "multi_priority")),
        };
        if !entrada.operacao.is_empty() && entrada.num < fim {
            saida.push(entrada);
        }
        resto = &depois[fecha + 1..];
    }
    saida
}

fn sem_extensao(nome: &str) -> String {
    let cortado = nome.rfind('.').map_or(nome, |i| &nome[..i]).trim();
    if cortado.is_empty() {
        nome.to_string()
    } else {
        cortado.to_string()
    }
}

/// O número como o JavaScript o escreve numa mensagem (`7`, e não `7.0`).
fn como_texto(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// É um arquivo do darktable? Pela extensão, e pelo conteúdo quando ela é
/// `.xmp`.
pub fn eh_do_darktable(texto: &str, nome_do_arquivo: &str) -> bool {
    nome_do_arquivo.to_lowercase().ends_with(".dtstyle")
        || texto.contains("<darktable_style")
        || texto.contains("darktable:history")
}

/// O estilo traduzido para os campos `dt_*`, com o que ficou de fora.
///
/// Devolve `None` quando o arquivo não tem histórico nenhum que se leia.
pub fn ler(texto: &str, nome_do_arquivo: &str) -> Option<PresetTraduzido> {
    let do_estilo = texto.contains("<darktable_style");
    let mut entradas = if do_estilo {
        entradas_do_estilo(texto)
    } else {
        entradas_do_xmp(texto)
    };
    if entradas.is_empty() {
        return None;
    }

    // 🔑 **A última entrada de cada instância é a que vale**, como no
    // darktable: o histórico de uma foto pode ter a mesma exposição mexida três
    // vezes. A posição é a da primeira vez que a instância apareceu — a de um
    // `Map` do JavaScript.
    entradas.sort_by(|a, b| a.num.total_cmp(&b.num));
    let mut vigentes: Vec<(String, Entrada)> = Vec::new();
    for entrada in entradas {
        let chave = format!("{}#{}", entrada.operacao, como_texto(entrada.prioridade));
        match vigentes.iter_mut().find(|(c, _)| *c == chave) {
            Some((_, vigente)) => *vigente = entrada,
            None => vigentes.push((chave, entrada)),
        }
    }

    let neutro = Ajustes::default().como_vetor();
    let mut ajustes = PresetAdjustments::vazia();
    let mut ignorados: Vec<String> = Vec::new();
    let mut ignorar = |motivo: String| {
        if !ignorados.contains(&motivo) {
            ignorados.push(motivo);
        }
    };

    for (_, e) in &vigentes {
        if !e.ativo {
            continue;
        }
        let ilegivel = || format!("{}: parâmetros ilegíveis", e.operacao);

        if let Some(conferir) = tubulacao(&e.operacao) {
            match decodificar(&e.params) {
                Some(bytes) => {
                    if let Some(aviso) = conferir(&Bytes(&bytes)) {
                        ignorar(aviso);
                    }
                }
                None => ignorar(ilegivel()),
            }
            continue;
        }
        let Some(modulo) = modulo(&e.operacao) else {
            ignorar(format!(
                "{} (módulo do darktable ainda não portado)",
                e.operacao
            ));
            continue;
        };
        if e.prioridade != 0.0 {
            ignorar(format!("{}: segunda instância do módulo", e.operacao));
            continue;
        }
        if e.versao != modulo.versao {
            ignorar(format!(
                "{} v{} (só a v{}, do darktable 5.6, é lida)",
                e.operacao,
                como_texto(e.versao),
                como_texto(modulo.versao)
            ));
            continue;
        }
        match motivo_da_mistura(&e.blend) {
            Ok(None) => {}
            Ok(Some(motivo)) => {
                ignorar(format!("{}: {motivo}", e.operacao));
                continue;
            }
            Err(()) => {
                ignorar(ilegivel());
                continue;
            }
        }
        let Some(bytes) = decodificar(&e.params) else {
            ignorar(ilegivel());
            continue;
        };
        if bytes.len() < modulo.tamanho {
            ignorar(ilegivel());
            continue;
        }
        let leitura = match (modulo.ler)(&Bytes(&bytes)) {
            Ok(leitura) => leitura,
            Err(motivo) => {
                ignorar(motivo);
                continue;
            }
        };
        for aviso in leitura.avisos {
            ignorar(aviso);
        }
        for (campo, valor) in leitura.ajustes {
            // ⚠️ **Só o que difere do neutro**, como toda predefinição — menos o
            // interruptor, que é o que diz "este módulo está no estilo". Em
            // `f32`, que é a precisão em que o darktable gravou.
            let posicao = Ajustes::NOMES.iter().position(|nome| *nome == campo);
            let no_neutro = posicao.is_some_and(|i| valor == neutro[i]);
            if campo.ends_with("_ativo") || !no_neutro {
                ajustes = ajustes.com(campo, valor);
            }
        }
    }

    let nome_do_estilo = if do_estilo {
        desescapar(elemento(elemento(texto, "info"), "name"))
            .trim()
            .to_string()
    } else {
        String::new()
    };
    Some(PresetTraduzido {
        nome: if nome_do_estilo.is_empty() {
            sem_extensao(nome_do_arquivo)
        } else {
            nome_do_estilo
        },
        ajustes,
        ignorados,
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    /// O estilo do estúdio, copiado de `docs/RecordarFotos P&B.dtstyle` do site.
    const ESTILO_DO_ESTUDIO: &str = include_str!("recordarfotos-pb.dtstyle");

    enum Campo {
        F(f32),
        I(i32),
        U(u32),
    }
    use Campo::{F, I, U};

    /// Bytes little-endian em hexadecimal — o `hex` do teste do site.
    fn hex(campos: &[Campo]) -> String {
        campos
            .iter()
            .flat_map(|campo| match campo {
                F(v) => v.to_le_bytes(),
                I(v) => v.to_le_bytes(),
                U(v) => v.to_le_bytes(),
            })
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Um `dt_develop_blend_params_t`: máscara, espaço, modo, parâmetro,
    /// opacidade.
    fn mistura(mascara: u32, modo: u32, opacidade: f32) -> String {
        hex(&[U(mascara), I(4), U(modo), F(0.0), F(opacidade)]) + &"00".repeat(400)
    }

    fn exposicao(ev: f32) -> String {
        hex(&[I(0), F(-0.0019), F(ev), F(50.0), F(-4.0), I(0), I(1)])
    }

    struct Plugin {
        op: &'static str,
        versao: u32,
        params: String,
        ativo: bool,
        blend: Option<String>,
        prioridade: u32,
    }

    fn plugin(op: &'static str, versao: u32, params: String) -> Plugin {
        Plugin {
            op,
            versao,
            params,
            ativo: true,
            blend: None,
            prioridade: 0,
        }
    }

    fn estilo(nome: &str, plugins: &[Plugin]) -> String {
        let corpo: String = plugins
            .iter()
            .enumerate()
            .map(|(i, p)| {
                format!(
                    "<plugin><num>{}</num><module>{}</module><operation>{}</operation>\
                     <op_params>{}</op_params><enabled>{}</enabled>\
                     <blendop_params>{}</blendop_params><blendop_version>14</blendop_version>\
                     <multi_priority>{}</multi_priority><multi_name></multi_name></plugin>",
                    i + 1,
                    p.versao,
                    p.op,
                    p.params,
                    u8::from(p.ativo),
                    p.blend.clone().unwrap_or_else(|| mistura(0, 0x18, 100.0)),
                    p.prioridade
                )
            })
            .collect();
        format!(
            "<?xml version=\"1.0\"?><darktable_style version=\"1.0\"><info><name>{nome}</name></info><style>{corpo}</style></darktable_style>"
        )
    }

    fn lido(texto: &str) -> PresetTraduzido {
        ler(texto, "x.dtstyle").expect("há histórico")
    }

    /// 🚨 **O arquivo de verdade, e não uma cópia montada no teste.** A
    /// predefinição do sistema "RecordarFotos P&B" foi gerada deste mesmo
    /// `.dtstyle`; importá-lo pela coluna tem de dar exatamente os mesmos campos
    /// — senão o estúdio teria duas versões do próprio estilo.
    #[test]
    fn o_estilo_do_estudio_importado_e_a_predefinicao_do_sistema() {
        assert!(eh_do_darktable(
            ESTILO_DO_ESTUDIO,
            "RecordarFotos P&B.dtstyle"
        ));
        let lido = ler(ESTILO_DO_ESTUDIO, "RecordarFotos P&B.dtstyle").expect("é um estilo");
        let do_sistema = use_cases::presets::presets_de_sistema()
            .into_iter()
            .find(|p| p.name == "RecordarFotos P&B")
            .expect("a predefinição do estúdio");

        assert_eq!(lido.nome, "RecordarFotos P&B");
        assert_eq!(lido.ajustes, do_sistema.adjustments);
        // basecurve e rgbcurve estão no estilo, mas desligados: o darktable não
        // os aplica, e o relatório não tem o que acusar.
        assert!(lido.ignorados.is_empty(), "{:?}", lido.ignorados);
    }

    #[test]
    fn modulo_desligado_nao_entra_e_nao_e_acusado() {
        let lido = lido(&estilo(
            "x",
            &[Plugin {
                ativo: false,
                ..plugin("rgbcurve", 1, "00".into())
            }],
        ));
        assert!(lido.ajustes.is_empty());
        assert!(lido.ignorados.is_empty());
    }

    #[test]
    fn modulo_ligado_que_o_motor_nao_tem_e_acusado_pelo_nome() {
        let lido = lido(&estilo(
            "x",
            &[
                plugin("exposure", 7, exposicao(0.5)),
                plugin("filmicrgb", 6, "00000000".into()),
            ],
        ));
        assert_eq!(lido.ajustes.get("dt_exposure_exposure"), Some(0.5));
        assert_eq!(
            lido.ignorados,
            ["filmicrgb (módulo do darktable ainda não portado)"]
        );
    }

    #[test]
    fn mistura_com_mascara_ou_opacidade_parcial_tira_o_modulo() {
        for (blend, motivo) in [
            (
                mistura(1, 0x18, 50.0),
                "exposure: opacidade abaixo de 100 %",
            ),
            (
                mistura(1 | 4, 0x18, 100.0),
                "exposure: máscara desenhada, paramétrica ou raster",
            ),
            (
                mistura(1, 0x04, 100.0),
                "exposure: modo de mistura diferente do normal",
            ),
        ] {
            let lido = lido(&estilo(
                "x",
                &[Plugin {
                    blend: Some(blend),
                    ..plugin("exposure", 7, exposicao(0.5))
                }],
            ));
            assert!(lido.ajustes.is_empty(), "{motivo}");
            assert_eq!(lido.ignorados, [motivo]);
        }
        // Máscara "uniforme" a 100 % é o módulo inteiro: entra.
        let inteiro = lido(&estilo(
            "x",
            &[Plugin {
                blend: Some(mistura(1, 0x18, 100.0)),
                ..plugin("exposure", 7, exposicao(0.5))
            }],
        ));
        assert_eq!(inteiro.ajustes.get("dt_exposure_ativo"), Some(1.0));
    }

    #[test]
    fn outra_versao_da_struct_e_segunda_instancia_ficam_de_fora() {
        let lido = lido(&estilo(
            "x",
            &[
                plugin("exposure", 6, exposicao(0.5)),
                Plugin {
                    prioridade: 1,
                    ..plugin("monochrome", 2, hex(&[F(0.0), F(0.0), F(2.0), F(0.0)]))
                },
            ],
        ));
        assert!(lido.ajustes.is_empty());
        assert_eq!(
            lido.ignorados,
            [
                "exposure v6 (só a v7, do darktable 5.6, é lida)",
                "monochrome: segunda instância do módulo",
            ]
        );
    }

    #[test]
    fn tubulacao_no_padrao_nao_vira_nada_e_orientacao_fixa_avisa() {
        let lido = lido(&estilo(
            "x",
            &[
                plugin("gamma", 1, "0000000000000000".into()),
                plugin("flip", 2, hex(&[I(5)])),
            ],
        ));
        assert!(lido.ajustes.is_empty());
        assert_eq!(
            lido.ignorados,
            ["flip: orientação fixa no estilo (use o enquadramento)"]
        );
    }

    #[test]
    fn parametro_estragado_e_acusado_sem_derrubar_o_resto() {
        let lido = lido(&estilo(
            "x",
            &[
                plugin("exposure", 7, "zz".into()),
                plugin("monochrome", 2, hex(&[F(1.0), F(0.0), F(2.0), F(0.0)])),
            ],
        ));
        assert_eq!(lido.ignorados, ["exposure: parâmetros ilegíveis"]);
        assert_eq!(lido.ajustes.get("dt_monochrome_ativo"), Some(1.0));
    }

    fn li(num: u32, ev: f32) -> String {
        format!(
            "<rdf:li darktable:num=\"{num}\" darktable:operation=\"exposure\" darktable:enabled=\"1\" \
             darktable:modversion=\"7\" darktable:params=\"{}\" darktable:multi_priority=\"0\" \
             darktable:blendop_version=\"14\" darktable:blendop_params=\"{}\"/>",
            exposicao(ev),
            mistura(0, 0x18, 100.0)
        )
    }

    fn xmp(fim: u32) -> String {
        format!(
            "<x:xmpmeta><rdf:RDF><rdf:Description darktable:history_end=\"{fim}\">\
             <darktable:history><rdf:Seq>{}{}{}</rdf:Seq></darktable:history>\
             </rdf:Description></rdf:RDF></x:xmpmeta>",
            li(0, 0.163),
            li(1, 0.363),
            li(2, 0.913)
        )
    }

    #[test]
    fn no_xmp_vale_a_ultima_entrada_ate_o_history_end() {
        assert!(eh_do_darktable(&xmp(2), "img_0040.JPG.xmp"));
        let ate_dois = ler(&xmp(2), "img_0040.JPG.xmp").expect("há histórico");
        assert_eq!(ate_dois.ajustes.get("dt_exposure_exposure"), Some(0.363));
        assert_eq!(ate_dois.nome, "img_0040.JPG");
        let todos = ler(&xmp(3), "img_0040.JPG.xmp").expect("há histórico");
        assert_eq!(todos.ajustes.get("dt_exposure_exposure"), Some(0.913));
    }

    #[test]
    fn o_xmp_do_lightroom_nao_e_confundido_com_o_do_darktable() {
        assert!(!eh_do_darktable(
            r#"<x:xmpmeta crs:Exposure2012="+0.50"/>"#,
            "preset.xmp"
        ));
        assert!(ler("<x/>", "vazio.dtstyle").is_none(), "sem histórico");
    }

    #[test]
    fn o_base64_do_darktable_volta_aos_bytes() {
        assert_eq!(base64("TWFu").as_deref(), Some(&b"Man"[..]));
        assert_eq!(base64("TWE=").as_deref(), Some(&b"Ma"[..]));
        assert_eq!(base64("TQ==").as_deref(), Some(&b"M"[..]));
        assert_eq!(base64("T!=="), None);
    }

    /// Todo campo que um módulo escreve existe no motor — a tabela é de texto,
    /// e um nome errado importaria "com sucesso" sem mover nada.
    #[test]
    fn nenhum_modulo_escreve_em_campo_inventado() {
        let bytes = vec![1u8; 200];
        for operacao in [
            "exposure",
            "shadhi",
            "monochrome",
            "vignette",
            "colorbalancergb",
        ] {
            let modulo = modulo(operacao).expect("portado");
            // Os bytes todos em 1 fazem os módulos com modo recusarem; o que
            // importa é a lista de campos, então força o caminho feliz.
            let mut dados = bytes.clone();
            dados[0..4].copy_from_slice(&0i32.to_le_bytes());
            dados[44..48].copy_from_slice(&1i32.to_le_bytes());
            dados[128..132].copy_from_slice(&1i32.to_le_bytes());
            let leitura = (modulo.ler)(&Bytes(&dados)).expect("lê");
            for (campo, _) in leitura.ajustes {
                assert!(
                    Ajustes::NOMES.contains(&campo),
                    "{operacao} escreve em `{campo}`"
                );
            }
        }
    }
}
