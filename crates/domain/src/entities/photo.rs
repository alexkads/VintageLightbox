//! Photo Entity
//!
//! Entidade central do domínio representando uma fotografia.
//! Implementado usando TDD.

use crate::{
    value_objects::{ColorLabel, FilePath, Flag, PhotoId, Rating},
    DomainResult,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::value_objects::PhotoMetadata;

/// Entidade Photo - representa uma fotografia no catálogo
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    /// Identificador único
    id: PhotoId,
    /// Caminho do arquivo
    file_path: FilePath,
    /// Classificação por estrelas (0-5)
    rating: Option<Rating>,
    /// Etiqueta de cor
    color_label: Option<ColorLabel>,
    /// Flag (Pick/Reject)
    flag: Option<Flag>,
    /// Data de importação
    imported_at: DateTime<Utc>,
    /// Data de última modificação
    modified_at: DateTime<Utc>,
    /// Flag de foto editada
    is_edited: bool,
    /// Metadados técnicos (EXIF)
    metadata: Option<PhotoMetadata>,
    /// Caminho do thumbnail [DEPRECATED: Use PreviewStorage service]
    thumbnail_path: Option<FilePath>,
    /// Caminho do preview (resolução otimizada para tela) [DEPRECATED: Use PreviewStorage service]
    preview_path: Option<FilePath>,
    /// Ajuste de exposição (persistence)
    edit_exposure: Option<f32>,
    /// Ajuste de contraste (persistence)
    edit_contrast: Option<f32>,
    /// Ajuste de temperatura (white balance warm/cool)
    edit_temperature: Option<f32>,
    /// Ajuste de tint (green/magenta)
    edit_tint: Option<f32>,
    /// Ajuste de highlights (bright areas)
    edit_highlights: Option<f32>,
    /// Ajuste de shadows (dark areas)
    edit_shadows: Option<f32>,
    /// Ajuste de whites (brightest whites)
    edit_whites: Option<f32>,
    /// Ajuste de blacks (darkest blacks)
    edit_blacks: Option<f32>,
    /// Ajuste de clarity (local contrast/sharpness)
    edit_clarity: Option<f32>,
    /// Ajuste de vibrance (intelligent saturation)
    edit_vibrance: Option<f32>,
    /// Ajuste de saturation (overall color intensity)
    edit_saturation: Option<f32>,
    /// Tone curve: Shadows adjustment (-100 to +100)
    edit_tone_curve_shadows: Option<f32>,
    /// Tone curve: Darks adjustment (-100 to +100)
    edit_tone_curve_darks: Option<f32>,
    /// Tone curve: Lights adjustment (-100 to +100)
    edit_tone_curve_lights: Option<f32>,
    /// Tone curve: Highlights adjustment (-100 to +100)
    edit_tone_curve_highlights: Option<f32>,
    /// SHA-256 hash of the file content (for duplicate detection)
    content_hash: Option<String>,
    /// HSL: Red channel saturation adjustment (-100 to +100)
    edit_hsl_red_sat: Option<f32>,
    /// HSL: Orange channel saturation adjustment (-100 to +100)
    edit_hsl_orange_sat: Option<f32>,
    /// HSL: Yellow channel saturation adjustment (-100 to +100)
    edit_hsl_yellow_sat: Option<f32>,
    /// HSL: Green channel saturation adjustment (-100 to +100)
    edit_hsl_green_sat: Option<f32>,
    /// HSL: Aqua channel saturation adjustment (-100 to +100)
    /// HSL: Aqua channel saturation adjustment (-100 to +100)
    edit_hsl_aqua_sat: Option<f32>,
    /// HSL: Blue channel saturation adjustment (-100 to +100)
    edit_hsl_blue_sat: Option<f32>,
    /// HSL: Purple channel saturation adjustment (-100 to +100)
    edit_hsl_purple_sat: Option<f32>,
    /// HSL: Magenta channel saturation adjustment (-100 to +100)
    edit_hsl_magenta_sat: Option<f32>,
    
    // --- HSL Hue (-100 to +100) ---
    edit_hsl_red_hue: Option<f32>,
    edit_hsl_orange_hue: Option<f32>,
    edit_hsl_yellow_hue: Option<f32>,
    edit_hsl_green_hue: Option<f32>,
    edit_hsl_aqua_hue: Option<f32>,
    edit_hsl_blue_hue: Option<f32>,
    edit_hsl_purple_hue: Option<f32>,
    edit_hsl_magenta_hue: Option<f32>,

    // --- HSL Luminance (-100 to +100) ---
    edit_hsl_red_lum: Option<f32>,
    edit_hsl_orange_lum: Option<f32>,
    edit_hsl_yellow_lum: Option<f32>,
    edit_hsl_green_lum: Option<f32>,
    edit_hsl_aqua_lum: Option<f32>,
    edit_hsl_blue_lum: Option<f32>,
    edit_hsl_purple_lum: Option<f32>,
    edit_hsl_magenta_lum: Option<f32>,

    // --- Lens Corrections ---
    /// Lens Distortion (-100 to +100)
    edit_lens_distortion: Option<f32>,
    /// Vignette Amount (-100 to +100)
    edit_lens_vignette_amount: Option<f32>,
    /// Vignette Midpoint (0 to 100)
    edit_lens_vignette_midpoint: Option<f32>,

    /// Noise reduction: Luminance (0 to 100)
    edit_nr_luminance: Option<f32>,
    /// Noise reduction: Color (0 to 100)
    edit_nr_color: Option<f32>,
    /// Sharpening: Amount (0.0 to 100.0)
    edit_sharpen_amount: Option<f32>,
    /// Sharpening: Radius (0.5 to 3.0)
    edit_sharpen_radius: Option<f32>,

    // --- Crop & Rotation ---
    edit_crop_x: Option<f32>,
    edit_crop_y: Option<f32>,
    edit_crop_width: Option<f32>,
    edit_crop_height: Option<f32>,
    edit_crop_rotation: Option<i32>,
    edit_crop_angle: Option<f32>,
    edit_crop_flip_h: Option<bool>,
    edit_crop_flip_v: Option<bool>,
    edit_crop_fill_mode: Option<u8>,
}

impl Photo {
    /// Cria uma nova foto gerando automaticamente um ID único
    pub fn new(file_path: FilePath) -> Self {
        Self::with_id(PhotoId::new(), file_path)
    }

    /// Reconstrói uma foto a partir de dados persistidos (uso interno/infraestrutura)
    pub fn reconstruct(
        id: PhotoId,
        file_path: FilePath,
        imported_at: DateTime<Utc>,
        modified_at: DateTime<Utc>,
        metadata: Option<PhotoMetadata>,
        rating: Option<Rating>,
        color_label: Option<ColorLabel>,
        flag: Option<Flag>,
        is_edited: bool,
        thumbnail_path: Option<FilePath>,
        preview_path: Option<FilePath>,
        edit_exposure: Option<f32>,
        edit_contrast: Option<f32>,
        edit_temperature: Option<f32>,
        edit_tint: Option<f32>,
        edit_highlights: Option<f32>,
        edit_shadows: Option<f32>,
        edit_whites: Option<f32>,
        edit_blacks: Option<f32>,
        edit_clarity: Option<f32>,
        edit_vibrance: Option<f32>,
        edit_saturation: Option<f32>,
        edit_tone_curve_shadows: Option<f32>,
        edit_tone_curve_darks: Option<f32>,
        edit_tone_curve_lights: Option<f32>,
        edit_tone_curve_highlights: Option<f32>,
        content_hash: Option<String>,
        edit_hsl_red_sat: Option<f32>,
        edit_hsl_orange_sat: Option<f32>,
        edit_hsl_yellow_sat: Option<f32>,
        edit_hsl_green_sat: Option<f32>,
        edit_hsl_aqua_sat: Option<f32>,
        edit_hsl_blue_sat: Option<f32>,
        edit_hsl_purple_sat: Option<f32>,
        edit_hsl_magenta_sat: Option<f32>,
        edit_hsl_red_hue: Option<f32>,
        edit_hsl_orange_hue: Option<f32>,
        edit_hsl_yellow_hue: Option<f32>,
        edit_hsl_green_hue: Option<f32>,
        edit_hsl_aqua_hue: Option<f32>,
        edit_hsl_blue_hue: Option<f32>,
        edit_hsl_purple_hue: Option<f32>,
        edit_hsl_magenta_hue: Option<f32>,
        edit_hsl_red_lum: Option<f32>,
        edit_hsl_orange_lum: Option<f32>,
        edit_hsl_yellow_lum: Option<f32>,
        edit_hsl_green_lum: Option<f32>,
        edit_hsl_aqua_lum: Option<f32>,
        edit_hsl_blue_lum: Option<f32>,
        edit_hsl_purple_lum: Option<f32>,
        edit_hsl_magenta_lum: Option<f32>,
        edit_lens_distortion: Option<f32>,
        edit_lens_vignette_amount: Option<f32>,
        edit_lens_vignette_midpoint: Option<f32>,
        edit_nr_luminance: Option<f32>,
        edit_nr_color: Option<f32>,
        edit_sharpen_amount: Option<f32>,
        edit_sharpen_radius: Option<f32>,
        edit_crop_x: Option<f32>,
        edit_crop_y: Option<f32>,
        edit_crop_width: Option<f32>,
        edit_crop_height: Option<f32>,
        edit_crop_rotation: Option<i32>,
        edit_crop_angle: Option<f32>,
        edit_crop_flip_h: Option<bool>,
        edit_crop_flip_v: Option<bool>,
        edit_crop_fill_mode: Option<u8>,
    ) -> Self {
        Self {
            id,
            file_path,
            imported_at,
            modified_at,
            metadata,
            rating,
            color_label,
            flag,
            is_edited,
            thumbnail_path,
            preview_path,
            edit_exposure,
            edit_contrast,
            edit_temperature,
            edit_tint,
            edit_highlights,
            edit_shadows,
            edit_whites,
            edit_blacks,
            edit_clarity,
            edit_vibrance,
            edit_saturation,
            edit_tone_curve_shadows,
            edit_tone_curve_darks,
            edit_tone_curve_lights,
            edit_tone_curve_highlights,
            content_hash,
            edit_hsl_red_sat,
            edit_hsl_orange_sat,
            edit_hsl_yellow_sat,
            edit_hsl_green_sat,
            edit_hsl_aqua_sat,
            edit_hsl_blue_sat,
            edit_hsl_purple_sat,
            edit_hsl_magenta_sat,
            edit_hsl_red_hue,
            edit_hsl_orange_hue,
            edit_hsl_yellow_hue,
            edit_hsl_green_hue,
            edit_hsl_aqua_hue,
            edit_hsl_blue_hue,
            edit_hsl_purple_hue,
            edit_hsl_magenta_hue,
            edit_hsl_red_lum,
            edit_hsl_orange_lum,
            edit_hsl_yellow_lum,
            edit_hsl_green_lum,
            edit_hsl_aqua_lum,
            edit_hsl_blue_lum,
            edit_hsl_purple_lum,
            edit_hsl_magenta_lum,
            edit_lens_distortion,
            edit_lens_vignette_amount,
            edit_lens_vignette_midpoint,
            edit_nr_luminance,
            edit_nr_color,
            edit_sharpen_amount,
            edit_sharpen_radius,
            edit_crop_x,
            edit_crop_y,
            edit_crop_width,
            edit_crop_height,
            edit_crop_rotation,
            edit_crop_angle,
            edit_crop_flip_h,
            edit_crop_flip_v,
            edit_crop_fill_mode,
        }
    }

    /// Cria uma nova foto com ID específico (para testes/migrações)
    pub fn with_id(id: PhotoId, file_path: FilePath) -> Self {
        let now = Utc::now();
        Self::reconstruct(
            id, file_path, now, now, None, None, None, None, false, None, None,
            None, None, None, None, None, None, None, None, None, None, None,
            None, None, None, None, None,
            None, None, None, None, None, None, None, None, // HSL Sat
            None, None, None, None, None, None, None, None, // HSL Hue
            None, None, None, None, None, None, None, None, // HSL Lum
            None, None, None, // Lens
            None, None, // NR (2 fields)
            None, None, // Sharpening (2 fields)
            None, None, None, None, None, None, None, None, None, // Crop & Rotation (9 fields)
        )
    }

    /// Cria uma foto para testes
    #[cfg(test)]
    pub fn new_test() -> Self {
        Self::new(FilePath::new_unchecked("/test/photo.jpg"))
    }

    /// Retorna o ID da foto
    pub fn id(&self) -> PhotoId {
        self.id
    }

    /// Retorna o caminho do arquivo
    pub fn file_path(&self) -> &FilePath {
        &self.file_path
    }

    /// Retorna o rating atual
    pub fn rating(&self) -> Option<Rating> {
        self.rating
    }

    /// Define o rating da foto
    pub fn rate(&mut self, rating: Rating) -> DomainResult<()> {
        self.rating = Some(rating);
        self.modified_at = Utc::now();
        Ok(())
    }

    /// Remove o rating da foto
    pub fn unrate(&mut self) {
        self.rating = None;
        self.modified_at = Utc::now();
    }

    /// Retorna a etiqueta de cor atual
    pub fn color_label(&self) -> Option<ColorLabel> {
        self.color_label
    }

    /// Define a etiqueta de cor
    pub fn set_color_label(&mut self, label: ColorLabel) {
        self.color_label = Some(label);
        self.modified_at = Utc::now();
    }

    /// Remove a etiqueta de cor
    pub fn remove_color_label(&mut self) {
        self.color_label = None;
        self.modified_at = Utc::now();
    }

    /// Retorna a flag atual
    pub fn flag(&self) -> Option<Flag> {
        self.flag
    }

    /// Define a flag
    pub fn set_flag(&mut self, flag: Flag) {
        self.flag = Some(flag);
        self.modified_at = Utc::now();
    }

    /// Remove a flag
    pub fn remove_flag(&mut self) {
        self.flag = None;
        self.modified_at = Utc::now();
    }

    /// Verifica se a foto tem flag
    pub fn has_flag(&self) -> bool {
        self.flag.is_some()
    }

    /// Retorna a data de importação
    pub fn imported_at(&self) -> DateTime<Utc> {
        self.imported_at
    }

    /// Retorna a data de última modificação
    pub fn modified_at(&self) -> DateTime<Utc> {
        self.modified_at
    }

    /// Verifica se a foto foi editada
    pub fn is_edited(&self) -> bool {
        self.is_edited
    }

    /// Marca a foto como editada
    pub fn mark_as_edited(&mut self) {
        self.is_edited = true;
        self.modified_at = Utc::now();
    }

    /// Marca a foto como não editada (reset)
    pub fn reset_edits(&mut self) {
        self.is_edited = false;
        self.modified_at = Utc::now();
    }

    /// Verifica se a foto tem rating
    pub fn has_rating(&self) -> bool {
        self.rating.is_some()
    }

    /// Verifica se a foto tem etiqueta de cor
    pub fn has_color_label(&self) -> bool {
        self.color_label.is_some()
    }

    /// Retorna o nome do arquivo
    pub fn file_name(&self) -> Option<&str> {
        self.file_path.file_name()
    }

    /// Retorna a extensão do arquivo
    pub fn extension(&self) -> Option<&str> {
        self.file_path.extension()
    }

    /// Retorna os metadados da foto
    pub fn metadata(&self) -> Option<&PhotoMetadata> {
        self.metadata.as_ref()
    }

    /// Define os metadados da foto
    pub fn set_metadata(&mut self, metadata: PhotoMetadata) {
        self.metadata = Some(metadata);
        self.modified_at = Utc::now();
    }

    /// Retorna o caminho do thumbnail
    pub fn thumbnail_path(&self) -> Option<&FilePath> {
        self.thumbnail_path.as_ref()
    }

    /// Define o caminho do thumbnail
    pub fn set_thumbnail_path(&mut self, path: FilePath) {
        self.thumbnail_path = Some(path);
        self.modified_at = Utc::now();
    }

    /// Retorna o caminho do preview
    pub fn preview_path(&self) -> Option<&FilePath> {
        self.preview_path.as_ref()
    }

    /// Define o caminho do preview
    pub fn set_preview_path(&mut self, path: FilePath) {
        self.preview_path = Some(path);
        self.modified_at = Utc::now();
    }

    /// Retorna o campo edit_exposure
    pub fn edit_exposure(&self) -> Option<f32> {
        self.edit_exposure
    }

    /// Retorna o campo edit_contrast
    pub fn edit_contrast(&self) -> Option<f32> {
        self.edit_contrast
    }

    /// Retorna o campo edit_temperature
    pub fn edit_temperature(&self) -> Option<f32> {
        self.edit_temperature
    }

    /// Retorna o campo edit_tint
    pub fn edit_tint(&self) -> Option<f32> {
        self.edit_tint
    }

    /// Retorna o campo edit_highlights
    pub fn edit_highlights(&self) -> Option<f32> {
        self.edit_highlights
    }

    /// Retorna o campo edit_shadows
    pub fn edit_shadows(&self) -> Option<f32> {
        self.edit_shadows
    }

    /// Retorna o campo edit_whites
    pub fn edit_whites(&self) -> Option<f32> {
        self.edit_whites
    }

    /// Retorna o campo edit_blacks
    pub fn edit_blacks(&self) -> Option<f32> {
        self.edit_blacks
    }

    /// Retorna o campo edit_clarity
    pub fn edit_clarity(&self) -> Option<f32> {
        self.edit_clarity
    }

    /// Retorna o campo edit_vibrance
    pub fn edit_vibrance(&self) -> Option<f32> {
        self.edit_vibrance
    }

    /// Retorna o campo edit_saturation
    pub fn edit_saturation(&self) -> Option<f32> {
        self.edit_saturation
    }

    /// Define os ajustes de edição e marca como editada
    pub fn set_edits(
        &mut self,
        exposure: Option<f32>,
        contrast: Option<f32>,
        temperature: Option<f32>,
        tint: Option<f32>,
        highlights: Option<f32>,
        shadows: Option<f32>,
        whites: Option<f32>,
        blacks: Option<f32>,
        clarity: Option<f32>,
        vibrance: Option<f32>,
        saturation: Option<f32>,
        tone_curve_shadows: Option<f32>,
        tone_curve_darks: Option<f32>,
        tone_curve_lights: Option<f32>,
        tone_curve_highlights: Option<f32>,
        hsl_red_sat: Option<f32>,
        hsl_orange_sat: Option<f32>,
        hsl_yellow_sat: Option<f32>,
        hsl_green_sat: Option<f32>,
        hsl_aqua_sat: Option<f32>,
        hsl_blue_sat: Option<f32>,
        hsl_purple_sat: Option<f32>,
        hsl_magenta_sat: Option<f32>,
        hsl_red_hue: Option<f32>,
        hsl_orange_hue: Option<f32>,
        hsl_yellow_hue: Option<f32>,
        hsl_green_hue: Option<f32>,
        hsl_aqua_hue: Option<f32>,
        hsl_blue_hue: Option<f32>,
        hsl_purple_hue: Option<f32>,
        hsl_magenta_hue: Option<f32>,
        hsl_red_lum: Option<f32>,
        hsl_orange_lum: Option<f32>,
        hsl_yellow_lum: Option<f32>,
        hsl_green_lum: Option<f32>,
        hsl_aqua_lum: Option<f32>,
        hsl_blue_lum: Option<f32>,
        hsl_purple_lum: Option<f32>,
        hsl_magenta_lum: Option<f32>,
        lens_distortion: Option<f32>,
        lens_vignette_amount: Option<f32>,
        lens_vignette_midpoint: Option<f32>,
        nr_luminance: Option<f32>,
        nr_color: Option<f32>,
        sharpen_amount: Option<f32>,
        sharpen_radius: Option<f32>,
        crop_x: Option<f32>,
        crop_y: Option<f32>,
        crop_width: Option<f32>,
        crop_height: Option<f32>,
        crop_rotation: Option<i32>,
        crop_angle: Option<f32>,
        crop_flip_h: Option<bool>,
        crop_flip_v: Option<bool>,
        crop_fill_mode: Option<u8>,
    ) -> DomainResult<()> {
        self.edit_exposure = exposure;
        self.edit_contrast = contrast;
        self.edit_temperature = temperature;
        self.edit_tint = tint;
        self.edit_highlights = highlights;
        self.edit_shadows = shadows;
        self.edit_whites = whites;
        self.edit_blacks = blacks;
        self.edit_clarity = clarity;
        self.edit_vibrance = vibrance;
        self.edit_saturation = saturation;
        self.edit_tone_curve_shadows = tone_curve_shadows;
        self.edit_tone_curve_darks = tone_curve_darks;
        self.edit_tone_curve_lights = tone_curve_lights;
        self.edit_tone_curve_highlights = tone_curve_highlights;
        self.edit_hsl_red_sat = hsl_red_sat;
        self.edit_hsl_orange_sat = hsl_orange_sat;
        self.edit_hsl_yellow_sat = hsl_yellow_sat;
        self.edit_hsl_green_sat = hsl_green_sat;
        self.edit_hsl_aqua_sat = hsl_aqua_sat;
        self.edit_hsl_blue_sat = hsl_blue_sat;
        self.edit_hsl_purple_sat = hsl_purple_sat;
        self.edit_hsl_magenta_sat = hsl_magenta_sat;
        self.edit_hsl_red_hue = hsl_red_hue;
        self.edit_hsl_orange_hue = hsl_orange_hue;
        self.edit_hsl_yellow_hue = hsl_yellow_hue;
        self.edit_hsl_green_hue = hsl_green_hue;
        self.edit_hsl_aqua_hue = hsl_aqua_hue;
        self.edit_hsl_blue_hue = hsl_blue_hue;
        self.edit_hsl_purple_hue = hsl_purple_hue;
        self.edit_hsl_magenta_hue = hsl_magenta_hue;
        self.edit_hsl_red_lum = hsl_red_lum;
        self.edit_hsl_orange_lum = hsl_orange_lum;
        self.edit_hsl_yellow_lum = hsl_yellow_lum;
        self.edit_hsl_green_lum = hsl_green_lum;
        self.edit_hsl_aqua_lum = hsl_aqua_lum;
        self.edit_hsl_blue_lum = hsl_blue_lum;
        self.edit_hsl_purple_lum = hsl_purple_lum;
        self.edit_hsl_magenta_lum = hsl_magenta_lum;
        self.edit_lens_distortion = lens_distortion;
        self.edit_lens_vignette_amount = lens_vignette_amount;
        self.edit_lens_vignette_midpoint = lens_vignette_midpoint;
        self.edit_nr_luminance = nr_luminance;
        self.edit_nr_color = nr_color;
        self.edit_sharpen_amount = sharpen_amount;
        self.edit_sharpen_radius = sharpen_radius;
        self.edit_crop_x = crop_x;
        self.edit_crop_y = crop_y;
        self.edit_crop_width = crop_width;
        self.edit_crop_height = crop_height;
        self.edit_crop_rotation = crop_rotation;
        self.edit_crop_angle = crop_angle;
        self.edit_crop_flip_h = crop_flip_h;
        self.edit_crop_flip_v = crop_flip_v;
        self.edit_crop_fill_mode = crop_fill_mode;

        self.is_edited = true;
        self.modified_at = Utc::now();

        Ok(())
    }


    /// Retorna o campo edit_tone_curve_shadows
    pub fn edit_tone_curve_shadows(&self) -> Option<f32> {
        self.edit_tone_curve_shadows
    }

    /// Retorna o campo edit_crop_x
    pub fn edit_crop_x(&self) -> Option<f32> {
        self.edit_crop_x
    }

    /// Retorna o campo edit_crop_y
    pub fn edit_crop_y(&self) -> Option<f32> {
        self.edit_crop_y
    }

    /// Retorna o campo edit_crop_width
    pub fn edit_crop_width(&self) -> Option<f32> {
        self.edit_crop_width
    }

    /// Retorna o campo edit_crop_height
    pub fn edit_crop_height(&self) -> Option<f32> {
        self.edit_crop_height
    }

    /// Retorna o campo edit_crop_rotation
    pub fn edit_crop_rotation(&self) -> Option<i32> {
        self.edit_crop_rotation
    }

    /// Retorna o campo edit_crop_angle
    pub fn edit_crop_angle(&self) -> Option<f32> {
        self.edit_crop_angle
    }

    /// Retorna o campo edit_crop_flip_h
    pub fn edit_crop_flip_h(&self) -> Option<bool> {
        self.edit_crop_flip_h
    }

    /// Retorna o campo edit_crop_flip_v
    pub fn edit_crop_flip_v(&self) -> Option<bool> {
        self.edit_crop_flip_v
    }

    /// Retorna o campo edit_crop_fill_mode
    pub fn edit_crop_fill_mode(&self) -> Option<u8> {
        self.edit_crop_fill_mode
    }

    /// Retorna o campo edit_tone_curve_darks
    pub fn edit_tone_curve_darks(&self) -> Option<f32> {
        self.edit_tone_curve_darks
    }

    /// Retorna o campo edit_tone_curve_lights
    pub fn edit_tone_curve_lights(&self) -> Option<f32> {
        self.edit_tone_curve_lights
    }

    /// Retorna o campo edit_tone_curve_highlights
    pub fn edit_tone_curve_highlights(&self) -> Option<f32> {
        self.edit_tone_curve_highlights
    }

    /// Retorna o campo edit_hsl_red_sat
    pub fn edit_hsl_red_sat(&self) -> Option<f32> {
        self.edit_hsl_red_sat
    }

    /// Retorna o campo edit_hsl_orange_sat
    pub fn edit_hsl_orange_sat(&self) -> Option<f32> {
        self.edit_hsl_orange_sat
    }

    /// Retorna o campo edit_hsl_yellow_sat
    pub fn edit_hsl_yellow_sat(&self) -> Option<f32> {
        self.edit_hsl_yellow_sat
    }

    /// Retorna o campo edit_hsl_green_sat
    pub fn edit_hsl_green_sat(&self) -> Option<f32> {
        self.edit_hsl_green_sat
    }

    /// Retorna o campo edit_hsl_aqua_sat
    pub fn edit_hsl_aqua_sat(&self) -> Option<f32> {
        self.edit_hsl_aqua_sat
    }

    /// Retorna o campo edit_hsl_blue_sat
    pub fn edit_hsl_blue_sat(&self) -> Option<f32> {
        self.edit_hsl_blue_sat
    }

    /// Retorna o campo edit_hsl_purple_sat
    pub fn edit_hsl_purple_sat(&self) -> Option<f32> {
        self.edit_hsl_purple_sat
    }

    /// Retorna o campo edit_hsl_magenta_sat
    pub fn edit_hsl_magenta_sat(&self) -> Option<f32> {
        self.edit_hsl_magenta_sat
    }

    /// Retorna o campo edit_nr_luminance
    pub fn edit_nr_luminance(&self) -> Option<f32> {
        self.edit_nr_luminance
    }

    /// Retorna o campo edit_nr_color
    pub fn edit_nr_color(&self) -> Option<f32> {
        self.edit_nr_color
    }

    /// Retorna o campo edit_sharpen_amount
    pub fn edit_sharpen_amount(&self) -> Option<f32> {
        self.edit_sharpen_amount
    }

    /// Retorna o campo edit_sharpen_radius
    pub fn edit_sharpen_radius(&self) -> Option<f32> {
        self.edit_sharpen_radius
    }

    /// Retorna o campo edit_hsl_red_hue
    pub fn edit_hsl_red_hue(&self) -> Option<f32> { self.edit_hsl_red_hue }
    pub fn edit_hsl_orange_hue(&self) -> Option<f32> { self.edit_hsl_orange_hue }
    pub fn edit_hsl_yellow_hue(&self) -> Option<f32> { self.edit_hsl_yellow_hue }
    pub fn edit_hsl_green_hue(&self) -> Option<f32> { self.edit_hsl_green_hue }
    pub fn edit_hsl_aqua_hue(&self) -> Option<f32> { self.edit_hsl_aqua_hue }
    pub fn edit_hsl_blue_hue(&self) -> Option<f32> { self.edit_hsl_blue_hue }
    pub fn edit_hsl_purple_hue(&self) -> Option<f32> { self.edit_hsl_purple_hue }
    pub fn edit_hsl_magenta_hue(&self) -> Option<f32> { self.edit_hsl_magenta_hue }

    /// Retorna o campo edit_hsl_red_lum
    pub fn edit_hsl_red_lum(&self) -> Option<f32> { self.edit_hsl_red_lum }
    pub fn edit_hsl_orange_lum(&self) -> Option<f32> { self.edit_hsl_orange_lum }
    pub fn edit_hsl_yellow_lum(&self) -> Option<f32> { self.edit_hsl_yellow_lum }
    pub fn edit_hsl_green_lum(&self) -> Option<f32> { self.edit_hsl_green_lum }
    pub fn edit_hsl_aqua_lum(&self) -> Option<f32> { self.edit_hsl_aqua_lum }
    pub fn edit_hsl_blue_lum(&self) -> Option<f32> { self.edit_hsl_blue_lum }
    pub fn edit_hsl_purple_lum(&self) -> Option<f32> { self.edit_hsl_purple_lum }
    pub fn edit_hsl_magenta_lum(&self) -> Option<f32> { self.edit_hsl_magenta_lum }

    /// Retorna o campo edit_lens_distortion
    pub fn edit_lens_distortion(&self) -> Option<f32> { self.edit_lens_distortion }
    /// Retorna o campo edit_lens_vignette_amount
    pub fn edit_lens_vignette_amount(&self) -> Option<f32> { self.edit_lens_vignette_amount }
    /// Retorna o campo edit_lens_vignette_midpoint
    pub fn edit_lens_vignette_midpoint(&self) -> Option<f32> { self.edit_lens_vignette_midpoint }



    /// Retorna o hash de conteúdo do arquivo (SHA-256)
    pub fn content_hash(&self) -> Option<&str> {
        self.content_hash.as_deref()
    }

    /// Define o hash de conteúdo do arquivo
    pub fn set_content_hash(&mut self, hash: String) {
        self.content_hash = Some(hash);
        self.modified_at = Utc::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 🔴 RED -> 🟢 GREEN -> 🔵 REFACTOR

    #[test]
    fn test_photo_creation() {
        // Arrange
        let id = PhotoId::new();
        let path = FilePath::new("/test/photo.jpg").unwrap();

        // Act
        let photo = Photo::with_id(id, path.clone());

        // Assert
        assert_eq!(photo.id(), id);
        assert_eq!(photo.file_path(), &path);
        assert_eq!(photo.rating(), None);
        assert_eq!(photo.color_label(), None);
        assert_eq!(photo.flag(), None);
        assert!(!photo.is_edited());
    }

    #[test]
    fn test_photo_rate() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        let result = photo.rate(Rating::FIVE);

        // Assert
        assert!(result.is_ok());
        assert_eq!(photo.rating(), Some(Rating::FIVE));
        assert!(photo.has_rating());
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_rate_multiple_times() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act
        photo.rate(Rating::THREE).unwrap();
        photo.rate(Rating::FIVE).unwrap();

        // Assert
        assert_eq!(photo.rating(), Some(Rating::FIVE));
    }

    #[test]
    fn test_photo_unrate() {
        // Arrange
        let mut photo = Photo::new_test();
        photo.rate(Rating::FOUR).unwrap();

        // Act
        photo.unrate();

        // Assert
        assert_eq!(photo.rating(), None);
        assert!(!photo.has_rating());
    }

    #[test]
    fn test_photo_set_color_label() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        photo.set_color_label(ColorLabel::Red);

        // Assert
        assert_eq!(photo.color_label(), Some(ColorLabel::Red));
        assert!(photo.has_color_label());
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_remove_color_label() {
        // Arrange
        let mut photo = Photo::new_test();
        photo.set_color_label(ColorLabel::Blue);

        // Act
        photo.remove_color_label();

        // Assert
        assert_eq!(photo.color_label(), None);
        assert!(!photo.has_color_label());
    }

    #[test]
    fn test_photo_set_flag() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        photo.set_flag(Flag::Pick);

        // Assert
        assert_eq!(photo.flag(), Some(Flag::Pick));
        assert!(photo.has_flag());
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_remove_flag() {
        // Arrange
        let mut photo = Photo::new_test();
        photo.set_flag(Flag::Reject);

        // Act
        photo.remove_flag();

        // Assert
        assert_eq!(photo.flag(), None);
        assert!(!photo.has_flag());
    }

    #[test]
    fn test_photo_mark_as_edited() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        photo.mark_as_edited();

        // Assert
        assert!(photo.is_edited());
        assert!(photo.modified_at() > original_modified);
    }

    #[test]
    fn test_photo_reset_edits() {
        // Arrange
        let mut photo = Photo::new_test();
        photo.mark_as_edited();

        // Act
        photo.reset_edits();

        // Assert
        assert!(!photo.is_edited());
    }

    #[test]
    fn test_photo_file_name() {
        // Arrange
        let photo = Photo::new(
            FilePath::new("/path/to/my_photo.jpg").unwrap(),
        );

        // Act
        let name = photo.file_name();

        // Assert
        assert_eq!(name, Some("my_photo.jpg"));
    }

    #[test]
    fn test_photo_extension() {
        // Arrange
        let photo1 = Photo::new(
            FilePath::new("/path/photo.jpg").unwrap(),
        );
        let photo2 = Photo::new(
            FilePath::new("/path/photo.RAW").unwrap(),
        );

        // Assert
        assert_eq!(photo1.extension(), Some("jpg"));
        assert_eq!(photo2.extension(), Some("RAW"));
    }

    #[test]
    fn test_photo_imported_and_modified_dates() {
        // Arrange & Act
        let photo = Photo::new_test();

        // Assert
        assert!(photo.imported_at() <= Utc::now());
        assert!(photo.modified_at() <= Utc::now());
        assert_eq!(photo.imported_at(), photo.modified_at());
    }

    #[test]
    fn test_photo_modification_updates_date() {
        // Arrange
        let mut photo = Photo::new_test();
        let original_modified = photo.modified_at();

        // Act
        std::thread::sleep(std::time::Duration::from_millis(10));
        photo.rate(Rating::THREE).unwrap();

        // Assert
        assert!(photo.modified_at() > original_modified);
    }

    // TODO: These tests need update for new Photo::reconstruct/set_edits signatures
    // #[test]
    // fn test_photo_equality() { ... }
    // #[test]
    // fn test_set_edits() { ... }

    #[test]
    fn test_photo_clone() {
        // Arrange
        let mut photo1 = Photo::new_test();
        photo1.rate(Rating::FIVE).unwrap();
        photo1.set_color_label(ColorLabel::Green);
        photo1.set_flag(Flag::Pick);

        // Act
        let photo2 = photo1.clone();

        // Assert
        assert_eq!(photo1, photo2);
        assert_eq!(photo2.rating(), Some(Rating::FIVE));
        assert_eq!(photo2.color_label(), Some(ColorLabel::Green));
        assert_eq!(photo2.flag(), Some(Flag::Pick));
    }

    // 🔴 RED: Tests for Tone Curve fields
    #[test]
    fn test_tone_curve_fields_default_none() {
        // Arrange & Act
        let photo = Photo::new_test();

        // Assert
        assert!(photo.edit_tone_curve_shadows().is_none());
        assert!(photo.edit_tone_curve_darks().is_none());
        assert!(photo.edit_tone_curve_lights().is_none());
        assert!(photo.edit_tone_curve_highlights().is_none());
    }

    // TODO: These tests need update for new set_edits signature
    // test_set_tone_curve_via_set_edits
    // test_tone_curve_all_zones
    // test_tone_curve_with_other_edits
    // test_tone_curve_reconstruct_roundtrip
}

#[cfg(test)]
mod business_logic_tests {
    use super::*;

    #[test]
    fn test_rating_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act & Assert - Workflow completo de rating
        assert!(!photo.has_rating());
        
        photo.rate(Rating::THREE).unwrap();
        assert!(photo.has_rating());
        assert_eq!(photo.rating(), Some(Rating::THREE));

        photo.rate(Rating::FIVE).unwrap();
        assert_eq!(photo.rating(), Some(Rating::FIVE));

        photo.unrate();
        assert!(!photo.has_rating());
    }

    #[test]
    fn test_color_label_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act & Assert
        assert!(!photo.has_color_label());

        photo.set_color_label(ColorLabel::Red);
        assert!(photo.has_color_label());
        assert_eq!(photo.color_label(), Some(ColorLabel::Red));

        photo.set_color_label(ColorLabel::Blue);
        assert_eq!(photo.color_label(), Some(ColorLabel::Blue));

        photo.remove_color_label();
        assert!(!photo.has_color_label());
    }

    #[test]
    fn test_flag_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act & Assert
        assert!(!photo.has_flag());

        photo.set_flag(Flag::Pick);
        assert!(photo.has_flag());
        assert_eq!(photo.flag(), Some(Flag::Pick));

        photo.set_flag(Flag::Reject);
        assert_eq!(photo.flag(), Some(Flag::Reject));

        photo.remove_flag();
        assert!(!photo.has_flag());
    }

    #[test]
    fn test_edit_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act & Assert
        assert!(!photo.is_edited());

        photo.mark_as_edited();
        assert!(photo.is_edited());

        photo.reset_edits();
        assert!(!photo.is_edited());
    }

    #[test]
    fn test_combined_workflow() {
        // Arrange
        let mut photo = Photo::new_test();

        // Act - Simular workflow real de usuário
        photo.rate(Rating::FOUR).unwrap();
        photo.set_color_label(ColorLabel::Green);
        photo.set_flag(Flag::Pick);
        photo.mark_as_edited();

        // Assert
        assert_eq!(photo.rating(), Some(Rating::FOUR));
        assert_eq!(photo.color_label(), Some(ColorLabel::Green));
        assert!(photo.is_edited());
        assert!(photo.has_rating());
        assert!(photo.has_color_label());
    }
}
