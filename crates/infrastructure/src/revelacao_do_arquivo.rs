//! A revelação que o RAW já traz do Lightroom — e que vira os parâmetros da
//! foto na importação.
//!
//! # O que o fotógrafo quer
//!
//! Revelou no Lightroom, exportou o DNG (ou deixou o `.xmp` ao lado do NEF) e
//! trouxe para cá. Até 30/set/2026 a foto entrava com os controles no neutro e
//! a revelação ficava presa no arquivo: era refazê-la slider a slider.
//!
//! Agora a importação lê o XMP — de dentro do DNG ou do `.xmp` ao lado — e o
//! traduz pelo mesmo [`crate::lightroom`] das predefinições. O bruto continua
//! sendo o sensor, sem efeito (`raw_codec::decodificar`); o que vem do XMP são
//! **parâmetros**, e é por eles que a Revelação abre a foto como o Lightroom a
//! deixou.
//!
//! # 🚨 Só RAW
//!
//! Um JPEG exportado pelo Lightroom também traz `crs:` no XMP — e os pixels
//! dele **já estão revelados**. Aplicar os parâmetros por cima seria revelar
//! duas vezes. Por isso a leitura só acontece para RAW (`raw_codec::eh_raw`).
//!
//! # O corte
//!
//! `crs:CropLeft/Top/Right/Bottom` vêm em frações do **sensor**, antes da
//! orientação (a convenção do DNG SDK: o corte é aplicado à imagem e a
//! orientação depois). Aqui o retângulo é girado para a foto de pé, que é o
//! referencial do enquadramento deste motor.
//!
//! ⚠️ **Corte inclinado fica de fora, e avisado.** O retângulo do Lightroom com
//! `CropAngle` é medido na foto já girada; aplicar o retângulo sem o giro daria
//! outro enquadramento, e aplicar o giro com um sinal não conferido daria um
//! torto. O aviso é melhor que qualquer dos dois.

use std::path::{Path, PathBuf};

use domain::entities::Photo;
use domain::value_objects::CropSettings;

use crate::gpu_adjustments::Ajustes;
use crate::lightroom::{self, PresetBruto, Valor};

/// De onde veio o XMP.
#[derive(Debug, Clone, PartialEq)]
pub enum Origem {
    /// De dentro do próprio arquivo (o DNG que o Lightroom exporta).
    Embutida,
    /// Do `.xmp` ao lado do RAW (o que o Lightroom grava para NEF, CR2…).
    AoLado(PathBuf),
}

/// A revelação lida, já nos termos deste motor.
#[derive(Debug, Clone, PartialEq)]
pub struct RevelacaoDoArquivo {
    pub ajustes: Ajustes,
    pub corte: CropSettings,
    /// O que o Lightroom fez e este motor não tem — para o relatório.
    pub ignorados: Vec<String>,
    pub origem: Origem,
}

/// Os rótulos de balanço de branco do tradutor de predefinições.
///
/// 🔑 **Numa predefinição eles são aviso; num RAW, quase sempre ruído.** Todo
/// XMP de foto traz `Temperature="5250"`: é o balanço **da câmera**, o mesmo que
/// a revelação do sensor já usa. Só um balanço mudado no Lightroom
/// (`WhiteBalance` diferente de `As Shot`) é trabalho que ficou de fora.
const ROTULOS_DO_BALANCO: [&str; 2] = [
    "temperatura (depende do balanço da foto)",
    "matiz do balanço de branco",
];

/// O `.xmp` ao lado de um RAW: `DSC_1.xmp` (Lightroom) ou `DSC_1.NEF.xmp`.
pub fn xmp_ao_lado(caminho: &Path) -> Option<PathBuf> {
    let pasta = caminho.parent()?;
    let nome = caminho.file_name()?.to_string_lossy().to_string();
    let base = caminho.file_stem()?.to_string_lossy().to_string();
    [
        format!("{base}.xmp"),
        format!("{base}.XMP"),
        format!("{nome}.xmp"),
        format!("{nome}.XMP"),
    ]
    .into_iter()
    .map(|n| pasta.join(n))
    .find(|p| p.is_file())
}

/// A revelação do Lightroom que acompanha este arquivo, se houver.
///
/// O XMP de dentro vence o de fora: o DNG exportado é a revelação daquele
/// momento, e um `.xmp` com o mesmo nome ao lado dele é de outro arquivo.
pub fn procurar(caminho: &Path) -> Option<RevelacaoDoArquivo> {
    let nome = caminho.file_name()?.to_string_lossy().to_string();
    if !raw_codec::eh_raw(&nome) {
        return None;
    }
    let bytes = std::fs::read(caminho).ok()?;
    let orientacao = raw_codec::orientacao_exif(&bytes);

    if let Some(texto) = raw_codec::xmp(&bytes) {
        if let Some(revelacao) = de_xmp(&texto, &nome, orientacao, Origem::Embutida) {
            return Some(revelacao);
        }
    }
    let lateral = xmp_ao_lado(caminho)?;
    let texto = std::fs::read_to_string(&lateral).ok()?;
    de_xmp(&texto, &nome, orientacao, Origem::AoLado(lateral))
}

/// Traduz um XMP do Camera Raw para os parâmetros da foto.
///
/// `None` quando o XMP não é do Lightroom ou não muda nada que este motor
/// aplique — uma foto importada sem revelação continua sem revelação.
pub fn de_xmp(
    texto: &str,
    nome: &str,
    orientacao: Option<u16>,
    origem: Origem,
) -> Option<RevelacaoDoArquivo> {
    let bruto = lightroom::ler_xmp(texto, nome)?;
    let traduzido = lightroom::traduzir(&bruto);

    let mut ignorados: Vec<String> = traduzido
        .ignorados
        .into_iter()
        .filter(|r| !ROTULOS_DO_BALANCO.contains(&r.as_str()))
        .collect();
    if let Some(Valor::Texto(balanco)) = bruto.ajustes.get("WhiteBalance") {
        if balanco != "As Shot" {
            ignorados.push(format!(
                "balanço de branco \"{balanco}\" (fica o da câmera)"
            ));
        }
    }

    let corte = match corte_do_xmp(&bruto, orientacao) {
        Ok(corte) => corte,
        Err(motivo) => {
            ignorados.push(motivo);
            None
        }
    };

    if traduzido.ajustes.is_empty() && corte.is_none() {
        return None;
    }

    let mut vetor = Ajustes::default().como_vetor();
    for (campo, valor) in traduzido.ajustes.iter() {
        if let Some(posicao) = Ajustes::NOMES.iter().position(|n| *n == campo) {
            vetor[posicao] = valor;
        }
    }
    let ajustes = Ajustes::de_vetor(&vetor).expect("o vetor saiu de `como_vetor`");
    ignorados.sort();
    ignorados.dedup();

    Some(RevelacaoDoArquivo {
        ajustes,
        corte: corte.unwrap_or_default(),
        ignorados,
        origem,
    })
}

fn numero(bruto: &PresetBruto, chave: &str) -> Option<f32> {
    match bruto.ajustes.get(chave) {
        Some(Valor::Numero(n)) => Some(*n),
        _ => None,
    }
}

/// O corte do Lightroom no referencial da foto de pé.
///
/// `Ok(None)` é "sem corte"; `Err` é o corte que existe e não entra, com o
/// rótulo do relatório.
fn corte_do_xmp(
    bruto: &PresetBruto,
    orientacao: Option<u16>,
) -> Result<Option<CropSettings>, String> {
    if !matches!(bruto.ajustes.get("HasCrop"), Some(Valor::Booleano(true))) {
        return Ok(None);
    }
    let (Some(esquerda), Some(topo), Some(direita), Some(baixo)) = (
        numero(bruto, "CropLeft"),
        numero(bruto, "CropTop"),
        numero(bruto, "CropRight"),
        numero(bruto, "CropBottom"),
    ) else {
        return Ok(None);
    };
    if numero(bruto, "CropAngle").is_some_and(|a| a.abs() > 0.01) {
        return Err("corte inclinado (refaça o enquadramento aqui)".into());
    }
    let Some(orientacao) = orientacao else {
        return Err("corte (orientação do arquivo desconhecida)".into());
    };

    // Os quatro cantos, levados do sensor para a foto de pé.
    let de_pe = |x: f32, y: f32| -> (f32, f32) {
        match orientacao {
            2 => (1.0 - x, y),
            3 => (1.0 - x, 1.0 - y),
            4 => (x, 1.0 - y),
            5 => (y, x),
            6 => (1.0 - y, x),
            7 => (1.0 - y, 1.0 - x),
            8 => (y, 1.0 - x),
            _ => (x, y),
        }
    };
    let cantos = [
        de_pe(esquerda, topo),
        de_pe(direita, topo),
        de_pe(esquerda, baixo),
        de_pe(direita, baixo),
    ];
    let min_x = cantos
        .iter()
        .map(|c| c.0)
        .fold(f32::MAX, f32::min)
        .clamp(0.0, 1.0);
    let max_x = cantos
        .iter()
        .map(|c| c.0)
        .fold(f32::MIN, f32::max)
        .clamp(0.0, 1.0);
    let min_y = cantos
        .iter()
        .map(|c| c.1)
        .fold(f32::MAX, f32::min)
        .clamp(0.0, 1.0);
    let max_y = cantos
        .iter()
        .map(|c| c.1)
        .fold(f32::MIN, f32::max)
        .clamp(0.0, 1.0);
    if max_x - min_x >= 0.999 && max_y - min_y >= 0.999 {
        return Ok(None);
    }
    Ok(Some(CropSettings::new(
        min_x,
        min_y,
        max_x - min_x,
        max_y - min_y,
        0,
        0.0,
        false,
        false,
    )))
}

/// Grava a revelação na foto que está entrando: as colunas dos ajustes, o
/// enquadramento e os parâmetros inteiros em JSON — o mesmo que a Revelação
/// grava ao salvar (`GravadorDoBanco`), para a foto abrir igual.
pub fn na_entidade(foto: &mut Photo, revelacao: &RevelacaoDoArquivo) {
    let a = &revelacao.ajustes;
    let c = &revelacao.corte;
    let _ = foto.set_edits(
        Some(a.exposure),
        Some(a.contrast),
        Some(a.temperature),
        Some(a.tint),
        Some(a.highlights),
        Some(a.shadows),
        Some(a.whites),
        Some(a.blacks),
        Some(a.clarity),
        Some(a.vibrance),
        Some(a.saturation),
        Some(a.tone_curve_shadows),
        Some(a.tone_curve_darks),
        Some(a.tone_curve_lights),
        Some(a.tone_curve_highlights),
        Some(a.hsl_red_sat),
        Some(a.hsl_orange_sat),
        Some(a.hsl_yellow_sat),
        Some(a.hsl_green_sat),
        Some(a.hsl_aqua_sat),
        Some(a.hsl_blue_sat),
        Some(a.hsl_purple_sat),
        Some(a.hsl_magenta_sat),
        Some(a.hsl_red_hue),
        Some(a.hsl_orange_hue),
        Some(a.hsl_yellow_hue),
        Some(a.hsl_green_hue),
        Some(a.hsl_aqua_hue),
        Some(a.hsl_blue_hue),
        Some(a.hsl_purple_hue),
        Some(a.hsl_magenta_hue),
        Some(a.hsl_red_lum),
        Some(a.hsl_orange_lum),
        Some(a.hsl_yellow_lum),
        Some(a.hsl_green_lum),
        Some(a.hsl_aqua_lum),
        Some(a.hsl_blue_lum),
        Some(a.hsl_purple_lum),
        Some(a.hsl_magenta_lum),
        Some(a.lens_distortion),
        Some(a.lens_vignette_amount),
        Some(a.lens_vignette_midpoint),
        Some(a.nr_luminance),
        Some(a.nr_color),
        Some(a.sharpen_amount),
        Some(a.sharpen_radius),
        Some(a.split_shadow_hue),
        Some(a.split_shadow_sat),
        Some(a.split_highlight_hue),
        Some(a.split_highlight_sat),
        Some(a.split_balance),
        Some(a.grain_amount),
        Some(a.grain_size),
        Some(c.crop_x()),
        Some(c.crop_y()),
        Some(c.crop_width()),
        Some(c.crop_height()),
        Some(c.rotation_90()),
        Some(c.angle()),
        Some(c.flip_horizontal()),
        Some(c.flip_vertical()),
    );
    foto.definir_parametros(Some(
        crate::pos_venda::parametros::ajustes_em_json(a, c).to_string(),
    ));
}

/// A porta da importação (`domain::services::LeitorDaRevelacaoDoArquivo`).
#[derive(Debug, Default, Clone, Copy)]
pub struct LeitorDoLightroom;

impl domain::services::LeitorDaRevelacaoDoArquivo for LeitorDoLightroom {
    fn aplicar(&self, origem: &Path, foto: &mut Photo) -> Option<Vec<String>> {
        let revelacao = procurar(origem)?;
        na_entidade(foto, &revelacao);
        Some(revelacao.ignorados)
    }

    fn levar_xmp_junto(&self, origem: &Path, destino: &Path, mover: bool) {
        if origem == destino {
            return;
        }
        let Some(xmp) = xmp_ao_lado(origem) else {
            return;
        };
        let Some(nome) = destino.file_stem() else {
            return;
        };
        let novo = destino.with_file_name(format!("{}.xmp", nome.to_string_lossy()));
        // ⚠️ Não sobrescreve: um `.xmp` que já está no destino é de outra foto
        // (ou desta, de uma importação anterior), e apagá-lo é perder trabalho.
        if novo.exists() {
            return;
        }
        if std::fs::copy(&xmp, &novo).is_ok() && mover {
            let _ = std::fs::remove_file(&xmp);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Trecho do XMP do `_CSF7953.dng` do acervo (Lightroom, 30/set/2026): P&B
    /// pelo HSL, sépia na tonalização, grão, e o corte gravado mas desligado.
    const XMP_DO_ACERVO: &str = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description
    crs:WhiteBalance="As Shot" crs:Temperature="5250" crs:Tint="-6"
    crs:Exposure2012="+0.46" crs:Highlights2012="-33" crs:Dehaze="0"
    crs:SaturationAdjustmentRed="-100" crs:SaturationAdjustmentBlue="-100"
    crs:SplitToningShadowHue="50" crs:SplitToningShadowSaturation="35"
    crs:SplitToningHighlightHue="40" crs:SplitToningBalance="-30"
    crs:GrainAmount="34" crs:GrainSize="28" crs:GrainFrequency="55"
    crs:HasCrop="False" crs:CropTop="0" crs:CropLeft="0.031518" crs:CropBottom="1"
    crs:CropRight="0.920406" crs:CropAngle="0"/></rdf:RDF></x:xmpmeta>"#;

    fn com(pares: &str) -> String {
        format!(r#"<x:xmpmeta><rdf:RDF><rdf:Description {pares}/></rdf:RDF></x:xmpmeta>"#)
    }

    #[test]
    fn o_xmp_do_acervo_vira_os_controles() {
        let r = de_xmp(XMP_DO_ACERVO, "_CSF7953.dng", Some(8), Origem::Embutida).unwrap();
        assert!((r.ajustes.exposure - 0.46).abs() < 1e-4);
        assert_eq!(r.ajustes.highlights, -33.0);
        assert_eq!(r.ajustes.hsl_red_sat, -100.0);
        assert_eq!(r.ajustes.split_shadow_hue, 50.0);
        assert_eq!(r.ajustes.split_shadow_sat, 35.0);
        assert_eq!(r.ajustes.grain_amount, 34.0);
        assert_eq!(r.corte, CropSettings::default(), "HasCrop=False: sem corte");
        // O balanço é o da câmera: não é aviso. A aspereza do grão é.
        assert_eq!(r.ignorados, vec!["aspereza do grão".to_string()]);
    }

    #[test]
    fn balanco_mudado_no_lightroom_e_avisado() {
        let texto = com(r#"crs:WhiteBalance="Custom" crs:Temperature="3200" crs:Exposure2012="1""#);
        let r = de_xmp(&texto, "a.NEF", Some(1), Origem::Embutida).unwrap();
        assert!(
            r.ignorados.iter().any(|i| i.contains("Custom")),
            "{:?}",
            r.ignorados
        );
    }

    #[test]
    fn xmp_sem_nada_que_o_motor_aplique_nao_e_revelacao() {
        let texto =
            com(r#"crs:WhiteBalance="As Shot" crs:Temperature="5250" crs:Exposure2012="0""#);
        assert_eq!(de_xmp(&texto, "a.NEF", Some(1), Origem::Embutida), None);
    }

    #[test]
    fn o_corte_da_foto_deitada_vale_como_esta() {
        let texto = com(
            r#"crs:HasCrop="True" crs:CropLeft="0.1" crs:CropTop="0.2" crs:CropRight="0.7" crs:CropBottom="0.9""#,
        );
        let r = de_xmp(&texto, "a.NEF", Some(1), Origem::Embutida).unwrap();
        assert!((r.corte.crop_x() - 0.1).abs() < 1e-6);
        assert!((r.corte.crop_y() - 0.2).abs() < 1e-6);
        assert!((r.corte.crop_width() - 0.6).abs() < 1e-6);
        assert!((r.corte.crop_height() - 0.7).abs() < 1e-6);
    }

    /// Orientação 8 (girar 270° no horário): a esquerda do sensor vira a base
    /// da foto de pé, e o topo do sensor vira a esquerda.
    #[test]
    fn o_corte_da_foto_em_retrato_gira_com_ela() {
        let texto = com(
            r#"crs:HasCrop="True" crs:CropLeft="0.1" crs:CropTop="0.0" crs:CropRight="1.0" crs:CropBottom="0.5""#,
        );
        let r = de_xmp(&texto, "a.NEF", Some(8), Origem::Embutida).unwrap();
        // (x, y) → (y, 1 − x): x ∈ [0,1; 1] vira y ∈ [0; 0,9]; y ∈ [0; 0,5] vira x.
        assert!((r.corte.crop_x() - 0.0).abs() < 1e-6);
        assert!((r.corte.crop_width() - 0.5).abs() < 1e-6);
        assert!((r.corte.crop_y() - 0.0).abs() < 1e-6);
        assert!((r.corte.crop_height() - 0.9).abs() < 1e-6);
    }

    #[test]
    fn corte_inclinado_fica_de_fora_e_avisado() {
        let texto = com(
            r#"crs:HasCrop="True" crs:CropLeft="0.1" crs:CropTop="0.1" crs:CropRight="0.9" crs:CropBottom="0.9" crs:CropAngle="2.5" crs:Exposure2012="1""#,
        );
        let r = de_xmp(&texto, "a.NEF", Some(1), Origem::Embutida).unwrap();
        assert_eq!(r.corte, CropSettings::default());
        assert!(r.ignorados.iter().any(|i| i.contains("inclinado")));
    }

    #[test]
    fn jpeg_nunca_le_a_revelacao() {
        let pasta = tempfile::tempdir().unwrap();
        let jpeg = pasta.path().join("exportada.jpg");
        std::fs::write(&jpeg, XMP_DO_ACERVO).unwrap();
        assert_eq!(
            procurar(&jpeg),
            None,
            "os pixels do JPEG já estão revelados"
        );
    }

    #[test]
    fn acha_o_xmp_ao_lado_do_raw() {
        let pasta = tempfile::tempdir().unwrap();
        let raw = pasta.path().join("DSC_0001.NEF");
        std::fs::write(&raw, b"nao importa: o XMP de dentro nao existe").unwrap();
        std::fs::write(pasta.path().join("DSC_0001.xmp"), XMP_DO_ACERVO).unwrap();
        let r = procurar(&raw).unwrap();
        assert_eq!(r.origem, Origem::AoLado(pasta.path().join("DSC_0001.xmp")));
        assert!((r.ajustes.exposure - 0.46).abs() < 1e-4);
    }

    #[test]
    fn na_entidade_grava_colunas_e_parametros() {
        let texto = com(
            r#"crs:HasCrop="True" crs:CropLeft="0.1" crs:CropTop="0.2" crs:CropRight="0.7" crs:CropBottom="0.9" crs:SplitToningShadowHue="50" crs:SplitToningShadowSaturation="35""#,
        );
        let r = de_xmp(&texto, "a.NEF", Some(1), Origem::Embutida).unwrap();
        let mut foto = Photo::new(domain::value_objects::FilePath::new("/cartao/a.NEF").unwrap());
        na_entidade(&mut foto, &r);
        assert_eq!(
            crate::gpu_adjustments::ajustes_da_entidade(&foto).split_shadow_hue,
            50.0
        );
        assert!((crate::transformacao::corte_da_entidade(&foto).crop_x() - 0.1).abs() < 1e-6);
        let json: serde_json::Value = serde_json::from_str(foto.parametros().unwrap()).unwrap();
        assert!((json["corte_largura"].as_f64().unwrap() - 0.6).abs() < 1e-6);
    }

    #[test]
    fn o_xmp_ao_lado_vai_junto_com_o_nome_novo() {
        use domain::services::LeitorDaRevelacaoDoArquivo;
        let origem = tempfile::tempdir().unwrap();
        let destino = tempfile::tempdir().unwrap();
        let raw = origem.path().join("DSC_0001.NEF");
        std::fs::write(&raw, b"raw").unwrap();
        std::fs::write(origem.path().join("DSC_0001.xmp"), XMP_DO_ACERVO).unwrap();
        let novo = destino.path().join("photo-2026-09-30-001.nef");

        LeitorDoLightroom.levar_xmp_junto(&raw, &novo, true);

        let levado = destino.path().join("photo-2026-09-30-001.xmp");
        assert_eq!(std::fs::read_to_string(&levado).unwrap(), XMP_DO_ACERVO);
        assert!(
            !origem.path().join("DSC_0001.xmp").exists(),
            "mover apaga o de origem"
        );
        std::fs::write(&novo, b"raw").unwrap();
        assert!(
            procurar(&novo).is_some(),
            "no destino, a foto continua achando a revelação dela"
        );
    }
}
