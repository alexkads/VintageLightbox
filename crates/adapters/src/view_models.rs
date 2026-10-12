use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PhotoViewModel {
    pub id: String,
    pub name: String,
    pub path: String,
    pub thumbnail_path: Option<String>,
    pub date: String,
    pub camera: String,
    pub exposure: String,
    /// Como a foto foi tirada, campo a campo — a linha embaixo do histograma
    /// da Revelação. Vazia na foto que só existe no site (sem EXIF aqui).
    #[serde(default)]
    pub captura: Captura,
    pub rating: i32,
    pub color_label: Option<String>,
    pub flag: Option<i32>,
    /// Levada no balcão — o que o pós-venda libera para download.
    pub comprada: bool,
    /// Onde a foto está no site. `None` = só existe aqui.
    ///
    /// 🔑 É o que a tela usa para saber se **há o que negociar**: a negociação do
    /// balcão se grava na foto do site, e uma foto que nunca subiu não tem em
    /// qual linha ser gravada.
    #[serde(default)]
    pub pos_venda_foto_id: Option<String>,
    /// De qual ensaio esta foto é. `None` = foto solta, do catálogo.
    #[serde(default)]
    pub sessao_id: Option<String>,
    /// **Todos** os ajustes da revelação, na ordem de `Ajustes::NOMES` do
    /// motor, quando a foto veio do site.
    ///
    /// 🚨 As colunas `edit_*` abaixo são só os 53 ajustes antigos. A revelação do
    /// site tem os módulos novos (`dt_*`), e sem este campo eles se perdiam no
    /// caminho: a foto revelada em sépia no site abria neutra no desktop, e
    /// assim ia para a tela do cliente (2026-09-17). `None` = valem as colunas.
    #[serde(default)]
    pub ajustes_completos: Option<Vec<f32>>,
    /// A revelação inteira de uma foto **do catálogo**, em JSON, como o banco a
    /// guarda (`photos.edit_parametros`, migrations 023 e 027). A tela a converte; este
    /// crate não conhece o motor. `None` = foto revelada antes da coluna, e
    /// valem as colunas `edit_*`.
    // O nome antigo do campo, para o que já foi gravado continuar lendo.
    #[serde(default, alias = "receita")]
    pub parametros: Option<String>,
    /// A revelação **local** (máscaras e retoques) em JSON, como o banco a
    /// guarda (`photos.edit_locais`, migration 024). `None` = sem máscara.
    #[serde(default)]
    pub locais: Option<String>,
    /// Comprada no site (ou sem arquivo): **não se revela**, como no editor
    /// do site (`editavel`). O cliente pode já ter baixado o original.
    ///
    /// ⚠️ Não é o `comprada` de cima, que junta a levada no balcão — essa
    /// continua revelável no site.
    #[serde(default)]
    pub revelacao_travada: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub edit_exposure: Option<f32>,
    pub edit_contrast: Option<f32>,
    pub edit_temperature: Option<f32>,
    pub edit_tint: Option<f32>,
    pub edit_highlights: Option<f32>,
    pub edit_shadows: Option<f32>,
    pub edit_whites: Option<f32>,
    pub edit_blacks: Option<f32>,
    pub edit_clarity: Option<f32>,
    pub edit_vibrance: Option<f32>,
    pub edit_saturation: Option<f32>,
    pub edit_tone_curve_shadows: Option<f32>,
    pub edit_tone_curve_darks: Option<f32>,
    pub edit_tone_curve_lights: Option<f32>,
    pub edit_tone_curve_highlights: Option<f32>,
    // HSL Saturation
    pub edit_hsl_red_sat: Option<f32>,
    pub edit_hsl_orange_sat: Option<f32>,
    pub edit_hsl_yellow_sat: Option<f32>,
    pub edit_hsl_green_sat: Option<f32>,
    pub edit_hsl_aqua_sat: Option<f32>,
    pub edit_hsl_blue_sat: Option<f32>,
    pub edit_hsl_purple_sat: Option<f32>,
    pub edit_hsl_magenta_sat: Option<f32>,
    // HSL Hue
    pub edit_hsl_red_hue: Option<f32>,
    pub edit_hsl_orange_hue: Option<f32>,
    pub edit_hsl_yellow_hue: Option<f32>,
    pub edit_hsl_green_hue: Option<f32>,
    pub edit_hsl_aqua_hue: Option<f32>,
    pub edit_hsl_blue_hue: Option<f32>,
    pub edit_hsl_purple_hue: Option<f32>,
    pub edit_hsl_magenta_hue: Option<f32>,
    // HSL Lum
    pub edit_hsl_red_lum: Option<f32>,
    pub edit_hsl_orange_lum: Option<f32>,
    pub edit_hsl_yellow_lum: Option<f32>,
    pub edit_hsl_green_lum: Option<f32>,
    pub edit_hsl_aqua_lum: Option<f32>,
    pub edit_hsl_blue_lum: Option<f32>,
    pub edit_hsl_purple_lum: Option<f32>,
    pub edit_hsl_magenta_lum: Option<f32>,
    // Lens
    pub edit_lens_distortion: Option<f32>,
    pub edit_lens_vignette_amount: Option<f32>,
    pub edit_lens_vignette_midpoint: Option<f32>,
    // NR
    pub edit_nr_luminance: Option<f32>,
    pub edit_nr_color: Option<f32>,
    // Sharpening
    pub edit_sharpen_amount: Option<f32>,
    pub edit_sharpen_radius: Option<f32>,
    pub edit_split_shadow_hue: Option<f32>,
    pub edit_split_shadow_sat: Option<f32>,
    pub edit_split_highlight_hue: Option<f32>,
    pub edit_split_highlight_sat: Option<f32>,
    pub edit_split_balance: Option<f32>,
    pub edit_grain_amount: Option<f32>,
    pub edit_grain_size: Option<f32>,
    // Crop & Rotation
    pub edit_crop_x: Option<f32>,
    pub edit_crop_y: Option<f32>,
    pub edit_crop_width: Option<f32>,
    pub edit_crop_height: Option<f32>,
    pub edit_crop_rotation: Option<i32>,
    pub edit_crop_angle: Option<f32>,
    pub edit_crop_flip_h: Option<bool>,
    pub edit_crop_flip_v: Option<bool>,
    /// A perspectiva guiada — sem coluna: vem da revelação em JSON (`corte_persp_*`
    /// e `corte_guiaN_*`). `None` = nenhuma.
    #[serde(skip)]
    pub edit_crop_perspectiva: Option<domain::value_objects::PerspectivaGuiada>,
    /// "Restringir ao conteúdo" — da revelação (`corte_restringir`). `None` =
    /// ligado, o padrão.
    #[serde(skip)]
    pub edit_crop_restringir: Option<bool>,
}

impl PhotoViewModel {
    /// A perspectiva e o "restringir" que a revelação em JSON traz, nos dois
    /// campos acima. Revelação ausente, ilegível ou sem as chaves deixa os dois
    /// em `None`.
    pub fn ler_perspectiva_dos_parametros(&mut self, parametros: &serde_json::Value) {
        let p = domain::value_objects::PerspectivaGuiada::de_json(parametros);
        self.edit_crop_perspectiva = (!p.e_neutra()).then_some(p);
        self.edit_crop_restringir =
            (!domain::value_objects::perspectiva::restringir_de_json(parametros)).then_some(false);
    }
}

/// Os dados da captura que a câmera grava no EXIF: ISO, distância focal,
/// abertura e velocidade — os quatro que o Lightroom mostra sob o histograma.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Captura {
    pub iso: Option<u32>,
    /// Em milímetros.
    pub focal: Option<f64>,
    /// O número f.
    pub abertura: Option<f64>,
    /// Como o EXIF a escreve: "1/160", "0.5", "2".
    pub velocidade: Option<String>,
}

impl Captura {
    /// Os rótulos na ordem do Lightroom — ISO, distância focal, abertura e
    /// velocidade —, só os que a foto tem. Sem nenhum, a lista vem vazia.
    pub fn rotulos(&self) -> Vec<String> {
        let mut rotulos = Vec::with_capacity(4);
        if let Some(iso) = self.iso.filter(|iso| *iso > 0) {
            rotulos.push(format!("ISO {iso}"));
        }
        if let Some(focal) = self.focal.filter(|f| f.is_finite() && *f > 0.0) {
            rotulos.push(format!("{} mm", sem_zero_no_fim(focal)));
        }
        if let Some(abertura) = self.abertura.filter(|f| f.is_finite() && *f > 0.0) {
            rotulos.push(format!("f/{}", sem_zero_no_fim(abertura)));
        }
        if let Some(velocidade) = self
            .velocidade
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            // O EXIF às vezes já traz a unidade ("1/160 s").
            let numero = velocidade.trim_end_matches('s').trim();
            rotulos.push(format!("{numero} s"));
        }
        rotulos
    }
}

/// Uma casa decimal, e nenhuma quando ela é zero: `f/11`, `f/2.8`, `27 mm`.
fn sem_zero_no_fim(valor: f64) -> String {
    let texto = format!("{valor:.1}");
    texto
        .strip_suffix(".0")
        .map(str::to_string)
        .unwrap_or(texto)
}

#[cfg(test)]
mod testes_da_captura {
    use super::Captura;

    #[test]
    fn os_quatro_rotulos_na_ordem_do_lightroom() {
        let captura = Captura {
            iso: Some(1000),
            focal: Some(27.0),
            abertura: Some(11.0),
            velocidade: Some("1/160".into()),
        };
        assert_eq!(captura.rotulos(), ["ISO 1000", "27 mm", "f/11", "1/160 s"]);
    }

    #[test]
    fn a_decimal_fica_quando_nao_e_zero_e_a_unidade_nao_dobra() {
        let captura = Captura {
            iso: None,
            focal: Some(52.5),
            abertura: Some(2.8),
            velocidade: Some("1/60 s".into()),
        };
        assert_eq!(captura.rotulos(), ["52.5 mm", "f/2.8", "1/60 s"]);
    }

    #[test]
    fn sem_exif_nao_ha_rotulo() {
        assert!(Captura::default().rotulos().is_empty());
        // O zero que a câmera grava quando não sabe não vira "ISO 0".
        let zerada = Captura {
            iso: Some(0),
            focal: Some(0.0),
            abertura: Some(0.0),
            velocidade: Some("  ".into()),
        };
        assert!(zerada.rotulos().is_empty());
    }
}

/// ViewModel de um arquivo listado na grade de importação
///
/// Não carrega bytes de miniatura: a grade
/// pede a imagem por fora, só das células visíveis. Aqui vem o que dá para saber lendo
/// pouco — o suficiente para ordenar, filtrar e decidir.
#[derive(Debug, Clone)]
pub struct ImportCandidateViewModel {
    pub file_path: String,
    /// Nome do arquivo, sem o caminho — é o que a célula mostra
    pub file_name: String,
    pub file_size: u64,
    pub is_raw: bool,
    pub camera: String,
    /// Data de captura no formato EXIF, ou vazio quando não há — é o que a
    /// célula mostra
    pub date_time: String,
    /// A chave que põe o ensaio na ordem em que foi fotografado: a data do
    /// disparo com o subsegundo normalizado, ou vazio quando não há data.
    ///
    /// 🔑 **Separada da de cima de propósito.** A grade mostra a data; quem
    /// ordena precisa do subsegundo, que numa rajada é a única coisa que
    /// distingue seis fotos do mesmo segundo. Ver
    /// `PhotoMetadata::chave_de_captura`.
    pub captura: String,
    pub dimensions: Option<String>,
}

/// ViewModel for duplicate check results
#[derive(Debug, Clone)]
pub struct DuplicateCheckViewModel {
    pub file_path: String,
    pub content_hash: String,
    pub is_duplicate: bool,
    pub existing_photo_path: Option<String>,
}

/// ViewModel for import progress events
#[derive(Debug, Clone)]
pub enum ImportProgressViewModel {
    Starting {
        total: usize,
    },
    Processing {
        index: usize,
        path: String,
    },
    Completed {
        photo_id: String,
        path: String,
        /// A revelação do Lightroom que entrou com a foto: `Some(o que ficou
        /// de fora)`; `None` quando o arquivo não trazia revelação.
        revelacao_do_lightroom: Option<Vec<String>>,
    },
    Failed {
        path: String,
        error: String,
    },
    DuplicateSkipped {
        path: String,
        existing_path: String,
    },
    Paused {
        completed: usize,
        remaining: usize,
    },
    Finished {
        successful: usize,
        failed: usize,
        skipped: usize,
    },
}
