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
    /// Data de captura no formato EXIF, ou vazio quando não há
    pub date_time: String,
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
