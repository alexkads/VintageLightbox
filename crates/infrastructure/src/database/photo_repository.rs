//! SQLite Photo Repository Implementation
//!
//! Implementação concreta do PhotoRepository usando SQLite via SQLx.

use async_trait::async_trait;
use domain::{
    entities::Photo,
    repositories::PhotoRepository,
    value_objects::{ColorLabel, FilePath, Flag, PhotoId, Rating},
    DomainError, DomainResult,
};
use sqlx::{Row, SqlitePool};

/// Implementação SQLite do PhotoRepository
pub struct PhotoRepositoryImpl {
    pool: SqlitePool,
}

impl PhotoRepositoryImpl {
    /// Cria uma nova instância do repository
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Helper para converter row do banco em Photo
    fn row_to_photo(row: &sqlx::sqlite::SqliteRow) -> DomainResult<Photo> {
        use chrono::{DateTime, Utc};
        use domain::value_objects::PhotoMetadata;

        let id_str: String = row
            .try_get("id")
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to get id: {}", e)))?;

        let file_path_str: String = row.try_get("file_path").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get file_path: {}", e))
        })?;

        let imported_at_str: String = row.try_get("imported_at").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get imported_at: {}", e))
        })?;

        let modified_at_str: String = row.try_get("modified_at").map_err(|e| {
            DomainError::InvalidOperation(format!("Failed to get modified_at: {}", e))
        })?;

        let rating_val: Option<i64> = row.try_get("rating").ok();
        let color_label_str: Option<String> = row.try_get("color_label").ok();
        let flag_val: Option<i32> = row.try_get("flag").ok();
        let comprada_em_str: Option<String> = row.try_get("comprada_em").ok();
        let pos_venda_foto_id: Option<String> = row.try_get("pos_venda_foto_id").ok().flatten();
        let sessao_id: Option<String> = row.try_get("sessao_id").ok().flatten();
        let is_edited: bool = row.try_get("is_edited").unwrap_or(false);
        let thumbnail_path_str: Option<String> = row.try_get("thumbnail_path").ok();
        let preview_path_str: Option<String> = row.try_get("preview_path").ok();
        let edit_exposure: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_exposure")
            .unwrap_or(None);
        let edit_contrast: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_contrast")
            .unwrap_or(None);

        let edit_temperature: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_temperature")
            .unwrap_or(None);
        let edit_tint: Option<f32> = row.try_get::<Option<f32>, _>("edit_tint").unwrap_or(None);
        let edit_highlights: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_highlights")
            .unwrap_or(None);
        let edit_shadows: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_shadows")
            .unwrap_or(None);
        let edit_whites: Option<f32> = row.try_get::<Option<f32>, _>("edit_whites").unwrap_or(None);
        let edit_blacks: Option<f32> = row.try_get::<Option<f32>, _>("edit_blacks").unwrap_or(None);
        let edit_clarity: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_clarity")
            .unwrap_or(None);
        let edit_vibrance: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_vibrance")
            .unwrap_or(None);
        let edit_saturation: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_saturation")
            .unwrap_or(None);
        let edit_tone_curve_shadows: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_tone_curve_shadows")
            .unwrap_or(None);
        let edit_tone_curve_darks: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_tone_curve_darks")
            .unwrap_or(None);
        let edit_tone_curve_lights: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_tone_curve_lights")
            .unwrap_or(None);
        let edit_tone_curve_highlights: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_tone_curve_highlights")
            .unwrap_or(None);
        let content_hash: Option<String> = row.try_get("content_hash").ok();

        // HSL saturation fields
        let edit_hsl_red_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_red_sat")
            .unwrap_or(None);
        let edit_hsl_orange_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_orange_sat")
            .unwrap_or(None);
        let edit_hsl_yellow_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_yellow_sat")
            .unwrap_or(None);
        let edit_hsl_green_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_green_sat")
            .unwrap_or(None);
        let edit_hsl_aqua_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_aqua_sat")
            .unwrap_or(None);
        let edit_hsl_blue_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_blue_sat")
            .unwrap_or(None);
        let edit_hsl_purple_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_purple_sat")
            .unwrap_or(None);
        let edit_hsl_magenta_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_magenta_sat")
            .unwrap_or(None);

        // HSL Hue fields
        let edit_hsl_red_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_red_hue")
            .unwrap_or(None);
        let edit_hsl_orange_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_orange_hue")
            .unwrap_or(None);
        let edit_hsl_yellow_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_yellow_hue")
            .unwrap_or(None);
        let edit_hsl_green_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_green_hue")
            .unwrap_or(None);
        let edit_hsl_aqua_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_aqua_hue")
            .unwrap_or(None);
        let edit_hsl_blue_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_blue_hue")
            .unwrap_or(None);
        let edit_hsl_purple_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_purple_hue")
            .unwrap_or(None);
        let edit_hsl_magenta_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_magenta_hue")
            .unwrap_or(None);

        // HSL Luminance fields
        let edit_hsl_red_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_red_lum")
            .unwrap_or(None);
        let edit_hsl_orange_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_orange_lum")
            .unwrap_or(None);
        let edit_hsl_yellow_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_yellow_lum")
            .unwrap_or(None);
        let edit_hsl_green_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_green_lum")
            .unwrap_or(None);
        let edit_hsl_aqua_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_aqua_lum")
            .unwrap_or(None);
        let edit_hsl_blue_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_blue_lum")
            .unwrap_or(None);
        let edit_hsl_purple_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_purple_lum")
            .unwrap_or(None);
        let edit_hsl_magenta_lum: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_hsl_magenta_lum")
            .unwrap_or(None);

        // Lens Correction fields
        let edit_lens_distortion: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_lens_distortion")
            .unwrap_or(None);
        let edit_lens_vignette_amount: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_lens_vignette_amount")
            .unwrap_or(None);
        let edit_lens_vignette_midpoint: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_lens_vignette_midpoint")
            .unwrap_or(None);
        // Noise Reduction fields
        let edit_nr_luminance: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_nr_luminance")
            .unwrap_or(None);
        let edit_nr_color: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_nr_color")
            .unwrap_or(None);
        // Sharpening fields
        let edit_sharpen_amount: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_sharpen_amount")
            .unwrap_or(None);
        let edit_sharpen_radius: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_sharpen_radius")
            .unwrap_or(None);

        // Tonalização e grão
        let edit_split_shadow_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_split_shadow_hue")
            .unwrap_or(None);
        let edit_split_shadow_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_split_shadow_sat")
            .unwrap_or(None);
        let edit_split_highlight_hue: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_split_highlight_hue")
            .unwrap_or(None);
        let edit_split_highlight_sat: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_split_highlight_sat")
            .unwrap_or(None);
        let edit_split_balance: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_split_balance")
            .unwrap_or(None);
        let edit_grain_amount: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_grain_amount")
            .unwrap_or(None);
        let edit_grain_size: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_grain_size")
            .unwrap_or(None);

        // Crop fields
        let edit_crop_x: Option<f32> = row.try_get::<Option<f32>, _>("edit_crop_x").unwrap_or(None);
        let edit_crop_y: Option<f32> = row.try_get::<Option<f32>, _>("edit_crop_y").unwrap_or(None);
        let edit_crop_width: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_crop_width")
            .unwrap_or(None);
        let edit_crop_height: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_crop_height")
            .unwrap_or(None);
        let edit_crop_rotation: Option<i32> = row
            .try_get::<Option<i32>, _>("edit_crop_rotation")
            .unwrap_or(None);
        let edit_crop_angle: Option<f32> = row
            .try_get::<Option<f32>, _>("edit_crop_angle")
            .unwrap_or(None);
        let edit_crop_flip_h: Option<bool> = row
            .try_get::<Option<bool>, _>("edit_crop_flip_h")
            .unwrap_or(None);
        let edit_crop_flip_v: Option<bool> = row
            .try_get::<Option<bool>, _>("edit_crop_flip_v")
            .unwrap_or(None);

        // Metadata persistido como JSON string
        let metadata_str: Option<String> = row.try_get("metadata").ok();
        let metadata: Option<PhotoMetadata> = match metadata_str {
            Some(s) => serde_json::from_str(&s).ok(), // Se falhar parse, ignora
            None => None,
        };

        // Parse values
        let photo_id = PhotoId::from_string(&id_str)?;
        let file_path = FilePath::new(&file_path_str)?;
        let rating = rating_val.and_then(|v| Rating::new(v as u8).ok());
        let color_label = color_label_str.and_then(|s| ColorLabel::from_name(&s).ok());
        let flag = flag_val.and_then(Flag::from_code);
        // Data ilegível vira "não comprada", e não erro: a linha inteira não pode
        // sumir da grade por causa de uma coluna acessória.
        let comprada_em = comprada_em_str
            .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
            .map(|d| d.with_timezone(&Utc));
        let thumbnail_path = thumbnail_path_str.and_then(|s| FilePath::new(&s).ok());
        let preview_path = preview_path_str.and_then(|s| FilePath::new(&s).ok());

        let imported_at = DateTime::parse_from_rfc3339(&imported_at_str)
            .map_err(|e| DomainError::InvalidOperation(format!("Invalid imported_at date: {}", e)))?
            .with_timezone(&Utc);

        let modified_at = DateTime::parse_from_rfc3339(&modified_at_str)
            .map_err(|e| DomainError::InvalidOperation(format!("Invalid modified_at date: {}", e)))?
            .with_timezone(&Utc);

        // Reconstruct photo with all fields
        let mut photo = Photo::reconstruct(
            photo_id,
            file_path,
            imported_at,
            modified_at,
            metadata,
            rating,
            color_label,
            flag,
            comprada_em,
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
            edit_split_shadow_hue,
            edit_split_shadow_sat,
            edit_split_highlight_hue,
            edit_split_highlight_sat,
            edit_split_balance,
            edit_grain_amount,
            edit_grain_size,
            edit_crop_x,
            edit_crop_y,
            edit_crop_width,
            edit_crop_height,
            edit_crop_rotation,
            edit_crop_angle,
            edit_crop_flip_h,
            edit_crop_flip_v,
        );

        // O id remoto entra fora da lista posicional — ver o comentário em
        // `Photo::reconstruct`.
        photo.definir_id_no_site(pos_venda_foto_id);
        photo.definir_sessao(sessao_id);
        Ok(photo)
    }
}

#[async_trait]
impl PhotoRepository for PhotoRepositoryImpl {
    async fn save(&self, photo: &Photo) -> DomainResult<()> {
        let id = photo.id().to_string();
        let file_path = photo.file_path().to_string_lossy().to_string();
        let rating = photo.rating().map(|r| r.value() as i64);
        let color_label = photo.color_label().map(|c| c.name().to_string());
        let flag = photo.flag().map(|f| f.as_code());
        let comprada_em = photo.comprada_em().map(|d| d.to_rfc3339());
        let pos_venda_foto_id = photo.id_no_site().map(str::to_string);
        let sessao_id = photo.sessao().map(str::to_string);
        let is_edited = photo.is_edited();
        let imported_at = photo.imported_at().to_rfc3339();
        let modified_at = photo.modified_at().to_rfc3339();
        let thumbnail_path = photo
            .thumbnail_path()
            .map(|p| p.to_string_lossy().to_string());
        let preview_path = photo
            .preview_path()
            .map(|p| p.to_string_lossy().to_string());
        let edit_exposure = photo.edit_exposure();
        let edit_contrast = photo.edit_contrast();
        let edit_temperature = photo.edit_temperature();
        let edit_tint = photo.edit_tint();
        let edit_highlights = photo.edit_highlights();
        let edit_shadows = photo.edit_shadows();
        let edit_whites = photo.edit_whites();
        let edit_blacks = photo.edit_blacks();
        let edit_clarity = photo.edit_clarity();
        let edit_vibrance = photo.edit_vibrance();
        let edit_saturation = photo.edit_saturation();
        let edit_tone_curve_shadows = photo.edit_tone_curve_shadows();
        let edit_tone_curve_darks = photo.edit_tone_curve_darks();
        let edit_tone_curve_lights = photo.edit_tone_curve_lights();
        let edit_tone_curve_highlights = photo.edit_tone_curve_highlights();
        let content_hash = photo.content_hash().map(|s| s.to_string());
        // HSL saturation fields
        let edit_hsl_red_sat = photo.edit_hsl_red_sat();
        let edit_hsl_orange_sat = photo.edit_hsl_orange_sat();
        let edit_hsl_yellow_sat = photo.edit_hsl_yellow_sat();
        let edit_hsl_green_sat = photo.edit_hsl_green_sat();
        let edit_hsl_aqua_sat = photo.edit_hsl_aqua_sat();
        let edit_hsl_blue_sat = photo.edit_hsl_blue_sat();
        let edit_hsl_purple_sat = photo.edit_hsl_purple_sat();
        let edit_hsl_magenta_sat = photo.edit_hsl_magenta_sat();
        // HSL Hue
        let edit_hsl_red_hue = photo.edit_hsl_red_hue();
        let edit_hsl_orange_hue = photo.edit_hsl_orange_hue();
        let edit_hsl_yellow_hue = photo.edit_hsl_yellow_hue();
        let edit_hsl_green_hue = photo.edit_hsl_green_hue();
        let edit_hsl_aqua_hue = photo.edit_hsl_aqua_hue();
        let edit_hsl_blue_hue = photo.edit_hsl_blue_hue();
        let edit_hsl_purple_hue = photo.edit_hsl_purple_hue();
        let edit_hsl_magenta_hue = photo.edit_hsl_magenta_hue();
        // HSL Lum
        let edit_hsl_red_lum = photo.edit_hsl_red_lum();
        let edit_hsl_orange_lum = photo.edit_hsl_orange_lum();
        let edit_hsl_yellow_lum = photo.edit_hsl_yellow_lum();
        let edit_hsl_green_lum = photo.edit_hsl_green_lum();
        let edit_hsl_aqua_lum = photo.edit_hsl_aqua_lum();
        let edit_hsl_blue_lum = photo.edit_hsl_blue_lum();
        let edit_hsl_purple_lum = photo.edit_hsl_purple_lum();
        let edit_hsl_magenta_lum = photo.edit_hsl_magenta_lum();
        // Lens
        let edit_lens_distortion = photo.edit_lens_distortion();
        let edit_lens_vignette_amount = photo.edit_lens_vignette_amount();
        let edit_lens_vignette_midpoint = photo.edit_lens_vignette_midpoint();
        // Noise Reduction fields
        let edit_nr_luminance = photo.edit_nr_luminance();
        let edit_nr_color = photo.edit_nr_color();
        let edit_sharpen_amount = photo.edit_sharpen_amount();
        let edit_sharpen_radius = photo.edit_sharpen_radius();
        // Tonalização e grão
        let edit_split_shadow_hue = photo.edit_split_shadow_hue();
        let edit_split_shadow_sat = photo.edit_split_shadow_sat();
        let edit_split_highlight_hue = photo.edit_split_highlight_hue();
        let edit_split_highlight_sat = photo.edit_split_highlight_sat();
        let edit_split_balance = photo.edit_split_balance();
        let edit_grain_amount = photo.edit_grain_amount();
        let edit_grain_size = photo.edit_grain_size();

        // Serializar metadata para JSON
        let metadata = photo.metadata().and_then(|m| serde_json::to_string(m).ok());

        sqlx::query(
            "INSERT INTO photos (id, file_path, rating, color_label, flag, comprada_em, pos_venda_foto_id, sessao_id, is_edited, imported_at, modified_at, metadata, thumbnail_path, preview_path, edit_exposure, edit_contrast, edit_temperature, edit_tint, edit_highlights, edit_shadows, edit_whites, edit_blacks, edit_clarity, edit_vibrance, edit_saturation, edit_tone_curve_shadows, edit_tone_curve_darks, edit_tone_curve_lights, edit_tone_curve_highlights, content_hash, edit_hsl_red_sat, edit_hsl_orange_sat, edit_hsl_yellow_sat, edit_hsl_green_sat, edit_hsl_aqua_sat, edit_hsl_blue_sat, edit_hsl_purple_sat, edit_hsl_magenta_sat, edit_hsl_red_hue, edit_hsl_orange_hue, edit_hsl_yellow_hue, edit_hsl_green_hue, edit_hsl_aqua_hue, edit_hsl_blue_hue, edit_hsl_purple_hue, edit_hsl_magenta_hue, edit_hsl_red_lum, edit_hsl_orange_lum, edit_hsl_yellow_lum, edit_hsl_green_lum, edit_hsl_aqua_lum, edit_hsl_blue_lum, edit_hsl_purple_lum, edit_hsl_magenta_lum, edit_lens_distortion, edit_lens_vignette_amount, edit_lens_vignette_midpoint, edit_nr_luminance, edit_nr_color, edit_sharpen_amount, edit_sharpen_radius, edit_split_shadow_hue, edit_split_shadow_sat, edit_split_highlight_hue, edit_split_highlight_sat, edit_split_balance, edit_grain_amount, edit_grain_size, edit_crop_x, edit_crop_y, edit_crop_width, edit_crop_height, edit_crop_rotation, edit_crop_angle, edit_crop_flip_h, edit_crop_flip_v)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&id)
        .bind(&file_path)
        .bind(rating)
        .bind(color_label)
        .bind(flag)
        .bind(&comprada_em)
        .bind(&pos_venda_foto_id)
        .bind(&sessao_id)
        .bind(is_edited)
        .bind(&imported_at)
        .bind(&modified_at)
        .bind(metadata)
        .bind(thumbnail_path)
        .bind(preview_path)
        .bind(edit_exposure)
        .bind(edit_contrast)
        .bind(edit_temperature)
        .bind(edit_tint)
        .bind(edit_highlights)
        .bind(edit_shadows)
        .bind(edit_whites)
        .bind(edit_blacks)
        .bind(edit_clarity)
        .bind(edit_vibrance)
        .bind(edit_saturation)
        .bind(edit_tone_curve_shadows)
        .bind(edit_tone_curve_darks)
        .bind(edit_tone_curve_lights)
        .bind(edit_tone_curve_highlights)
        .bind(content_hash)
        .bind(edit_hsl_red_sat)
        .bind(edit_hsl_orange_sat)
        .bind(edit_hsl_yellow_sat)
        .bind(edit_hsl_green_sat)
        .bind(edit_hsl_aqua_sat)
        .bind(edit_hsl_blue_sat)
        .bind(edit_hsl_purple_sat)
        .bind(edit_hsl_magenta_sat)
        .bind(edit_hsl_red_hue)
        .bind(edit_hsl_orange_hue)
        .bind(edit_hsl_yellow_hue)
        .bind(edit_hsl_green_hue)
        .bind(edit_hsl_aqua_hue)
        .bind(edit_hsl_blue_hue)
        .bind(edit_hsl_purple_hue)
        .bind(edit_hsl_magenta_hue)
        .bind(edit_hsl_red_lum)
        .bind(edit_hsl_orange_lum)
        .bind(edit_hsl_yellow_lum)
        .bind(edit_hsl_green_lum)
        .bind(edit_hsl_aqua_lum)
        .bind(edit_hsl_blue_lum)
        .bind(edit_hsl_purple_lum)
        .bind(edit_hsl_magenta_lum)
        .bind(edit_lens_distortion)
        .bind(edit_lens_vignette_amount)
        .bind(edit_lens_vignette_midpoint)
        .bind(edit_nr_luminance)
        .bind(edit_nr_color)
        .bind(edit_sharpen_amount)
        .bind(edit_sharpen_radius)
        .bind(edit_split_shadow_hue)
        .bind(edit_split_shadow_sat)
        .bind(edit_split_highlight_hue)
        .bind(edit_split_highlight_sat)
        .bind(edit_split_balance)
        .bind(edit_grain_amount)
        .bind(edit_grain_size)
        // 🚨 **Os oito do enquadramento faltavam aqui até 2026-09-06.** O
        // `INSERT` lista 68 colunas e vinculava 60: os `edit_crop_*` ficavam com
        // o `?` sem valor, que no SQLite entra como NULL. Passava despercebido
        // porque foto recém-importada não tem enquadramento mesmo — mas `save`
        // de uma foto que já tinha um o apagava, calado.
        .bind(photo.edit_crop_x())
        .bind(photo.edit_crop_y())
        .bind(photo.edit_crop_width())
        .bind(photo.edit_crop_height())
        .bind(photo.edit_crop_rotation())
        .bind(photo.edit_crop_angle())
        .bind(photo.edit_crop_flip_h())
        .bind(photo.edit_crop_flip_v())
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Failed to save photo: {}", e)))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &PhotoId) -> DomainResult<Option<Photo>> {
        let id_str = id.to_string();

        let row = sqlx::query("SELECT * FROM photos WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to find photo: {}", e)))?;

        match row {
            Some(r) => Ok(Some(Self::row_to_photo(&r)?)),
            None => Ok(None),
        }
    }

    async fn find_all(&self) -> DomainResult<Vec<Photo>> {
        let rows = sqlx::query("SELECT * FROM photos ORDER BY imported_at DESC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to fetch photos: {}", e)))?;

        rows.iter().map(Self::row_to_photo).collect()
    }

    async fn update(&self, photo: &Photo) -> DomainResult<()> {
        let id = photo.id().to_string();
        let file_path = photo.file_path().to_string_lossy().to_string();
        let rating = photo.rating().map(|r| r.value() as i64);
        let color_label = photo.color_label().map(|c| c.name().to_string());
        let flag = photo.flag().map(|f| f.as_code());
        let comprada_em = photo.comprada_em().map(|d| d.to_rfc3339());
        let pos_venda_foto_id = photo.id_no_site().map(str::to_string);
        let sessao_id = photo.sessao().map(str::to_string);
        let is_edited = photo.is_edited();
        let modified_at = photo.modified_at().to_rfc3339();
        let thumbnail_path = photo
            .thumbnail_path()
            .map(|p| p.to_string_lossy().to_string());
        let preview_path = photo
            .preview_path()
            .map(|p| p.to_string_lossy().to_string());
        let edit_exposure = photo.edit_exposure();
        let edit_contrast = photo.edit_contrast();
        let edit_temperature = photo.edit_temperature();
        let edit_tint = photo.edit_tint();
        let edit_highlights = photo.edit_highlights();
        let edit_shadows = photo.edit_shadows();
        let edit_whites = photo.edit_whites();
        let edit_blacks = photo.edit_blacks();
        let edit_clarity = photo.edit_clarity();
        let edit_vibrance = photo.edit_vibrance();
        let edit_saturation = photo.edit_saturation();
        let edit_tone_curve_shadows = photo.edit_tone_curve_shadows();
        let edit_tone_curve_darks = photo.edit_tone_curve_darks();
        let edit_tone_curve_lights = photo.edit_tone_curve_lights();
        let edit_tone_curve_highlights = photo.edit_tone_curve_highlights();
        let content_hash = photo.content_hash().map(|s| s.to_string());
        // HSL saturation fields
        let edit_hsl_red_sat = photo.edit_hsl_red_sat();
        let edit_hsl_orange_sat = photo.edit_hsl_orange_sat();
        let edit_hsl_yellow_sat = photo.edit_hsl_yellow_sat();
        let edit_hsl_green_sat = photo.edit_hsl_green_sat();
        let edit_hsl_aqua_sat = photo.edit_hsl_aqua_sat();
        let edit_hsl_blue_sat = photo.edit_hsl_blue_sat();
        let edit_hsl_purple_sat = photo.edit_hsl_purple_sat();
        let edit_hsl_magenta_sat = photo.edit_hsl_magenta_sat();
        // HSL Hue
        let edit_hsl_red_hue = photo.edit_hsl_red_hue();
        let edit_hsl_orange_hue = photo.edit_hsl_orange_hue();
        let edit_hsl_yellow_hue = photo.edit_hsl_yellow_hue();
        let edit_hsl_green_hue = photo.edit_hsl_green_hue();
        let edit_hsl_aqua_hue = photo.edit_hsl_aqua_hue();
        let edit_hsl_blue_hue = photo.edit_hsl_blue_hue();
        let edit_hsl_purple_hue = photo.edit_hsl_purple_hue();
        let edit_hsl_magenta_hue = photo.edit_hsl_magenta_hue();
        // HSL Lum
        let edit_hsl_red_lum = photo.edit_hsl_red_lum();
        let edit_hsl_orange_lum = photo.edit_hsl_orange_lum();
        let edit_hsl_yellow_lum = photo.edit_hsl_yellow_lum();
        let edit_hsl_green_lum = photo.edit_hsl_green_lum();
        let edit_hsl_aqua_lum = photo.edit_hsl_aqua_lum();
        let edit_hsl_blue_lum = photo.edit_hsl_blue_lum();
        let edit_hsl_purple_lum = photo.edit_hsl_purple_lum();
        let edit_hsl_magenta_lum = photo.edit_hsl_magenta_lum();
        // Lens
        let edit_lens_distortion = photo.edit_lens_distortion();
        let edit_lens_vignette_amount = photo.edit_lens_vignette_amount();
        let edit_lens_vignette_midpoint = photo.edit_lens_vignette_midpoint();
        // Noise Reduction fields
        let edit_nr_luminance = photo.edit_nr_luminance();
        let edit_nr_color = photo.edit_nr_color();
        // Sharpening fields
        let edit_sharpen_amount = photo.edit_sharpen_amount();
        let edit_sharpen_radius = photo.edit_sharpen_radius();
        // Tonalização e grão
        let edit_split_shadow_hue = photo.edit_split_shadow_hue();
        let edit_split_shadow_sat = photo.edit_split_shadow_sat();
        let edit_split_highlight_hue = photo.edit_split_highlight_hue();
        let edit_split_highlight_sat = photo.edit_split_highlight_sat();
        let edit_split_balance = photo.edit_split_balance();
        let edit_grain_amount = photo.edit_grain_amount();
        let edit_grain_size = photo.edit_grain_size();
        // Crop fields
        let edit_crop_x = photo.edit_crop_x();
        let edit_crop_y = photo.edit_crop_y();
        let edit_crop_width = photo.edit_crop_width();
        let edit_crop_height = photo.edit_crop_height();
        let edit_crop_rotation = photo.edit_crop_rotation();
        let edit_crop_angle = photo.edit_crop_angle();
        let edit_crop_flip_h = photo.edit_crop_flip_h();
        let edit_crop_flip_v = photo.edit_crop_flip_v();

        // Serializar metadata para JSON
        let metadata = photo.metadata().and_then(|m| serde_json::to_string(m).ok());

        let result = sqlx::query(
            "UPDATE photos
             SET file_path = ?, rating = ?, color_label = ?, flag = ?, comprada_em = ?, pos_venda_foto_id = ?, sessao_id = ?, is_edited = ?, modified_at = ?, metadata = ?, thumbnail_path = ?, preview_path = ?, edit_exposure = ?, edit_contrast = ?, edit_temperature = ?, edit_tint = ?, edit_highlights = ?, edit_shadows = ?, edit_whites = ?, edit_blacks = ?, edit_clarity = ?, edit_vibrance = ?, edit_saturation = ?, edit_tone_curve_shadows = ?, edit_tone_curve_darks = ?, edit_tone_curve_lights = ?, edit_tone_curve_highlights = ?, content_hash = ?, edit_hsl_red_sat = ?, edit_hsl_orange_sat = ?, edit_hsl_yellow_sat = ?, edit_hsl_green_sat = ?, edit_hsl_aqua_sat = ?, edit_hsl_blue_sat = ?, edit_hsl_purple_sat = ?, edit_hsl_magenta_sat = ?, edit_hsl_red_hue = ?, edit_hsl_orange_hue = ?, edit_hsl_yellow_hue = ?, edit_hsl_green_hue = ?, edit_hsl_aqua_hue = ?, edit_hsl_blue_hue = ?, edit_hsl_purple_hue = ?, edit_hsl_magenta_hue = ?, edit_hsl_red_lum = ?, edit_hsl_orange_lum = ?, edit_hsl_yellow_lum = ?, edit_hsl_green_lum = ?, edit_hsl_aqua_lum = ?, edit_hsl_blue_lum = ?, edit_hsl_purple_lum = ?, edit_hsl_magenta_lum = ?, edit_lens_distortion = ?, edit_lens_vignette_amount = ?, edit_lens_vignette_midpoint = ?, edit_nr_luminance = ?, edit_nr_color = ?, edit_sharpen_amount = ?, edit_sharpen_radius = ?, edit_split_shadow_hue = ?, edit_split_shadow_sat = ?, edit_split_highlight_hue = ?, edit_split_highlight_sat = ?, edit_split_balance = ?, edit_grain_amount = ?, edit_grain_size = ?, edit_crop_x = ?, edit_crop_y = ?, edit_crop_width = ?, edit_crop_height = ?, edit_crop_rotation = ?, edit_crop_angle = ?, edit_crop_flip_h = ?, edit_crop_flip_v = ?
             WHERE id = ?"
        )
        .bind(&file_path)
        .bind(rating)
        .bind(color_label)
        .bind(flag)
        .bind(&comprada_em)
        .bind(&pos_venda_foto_id)
        .bind(&sessao_id)
        .bind(is_edited)
        .bind(&modified_at)
        .bind(metadata)
        .bind(thumbnail_path)
        .bind(preview_path)
        .bind(edit_exposure)
        .bind(edit_contrast)
        .bind(edit_temperature)
        .bind(edit_tint)
        .bind(edit_highlights)
        .bind(edit_shadows)
        .bind(edit_whites)
        .bind(edit_blacks)
        .bind(edit_clarity)
        .bind(edit_vibrance)
        .bind(edit_saturation)
        .bind(edit_tone_curve_shadows)
        .bind(edit_tone_curve_darks)
        .bind(edit_tone_curve_lights)
        .bind(edit_tone_curve_highlights)
        .bind(content_hash)
        .bind(edit_hsl_red_sat)
        .bind(edit_hsl_orange_sat)
        .bind(edit_hsl_yellow_sat)
        .bind(edit_hsl_green_sat)
        .bind(edit_hsl_aqua_sat)
        .bind(edit_hsl_blue_sat)
        .bind(edit_hsl_purple_sat)
        .bind(edit_hsl_magenta_sat)
        .bind(edit_hsl_red_hue)
        .bind(edit_hsl_orange_hue)
        .bind(edit_hsl_yellow_hue)
        .bind(edit_hsl_green_hue)
        .bind(edit_hsl_aqua_hue)
        .bind(edit_hsl_blue_hue)
        .bind(edit_hsl_purple_hue)
        .bind(edit_hsl_magenta_hue)
        .bind(edit_hsl_red_lum)
        .bind(edit_hsl_orange_lum)
        .bind(edit_hsl_yellow_lum)
        .bind(edit_hsl_green_lum)
        .bind(edit_hsl_aqua_lum)
        .bind(edit_hsl_blue_lum)
        .bind(edit_hsl_purple_lum)
        .bind(edit_hsl_magenta_lum)
        .bind(edit_lens_distortion)
        .bind(edit_lens_vignette_amount)
        .bind(edit_lens_vignette_midpoint)
        .bind(edit_nr_luminance)
        .bind(edit_nr_color)
        .bind(edit_sharpen_amount)
        .bind(edit_sharpen_radius)
        .bind(edit_split_shadow_hue)
        .bind(edit_split_shadow_sat)
        .bind(edit_split_highlight_hue)
        .bind(edit_split_highlight_sat)
        .bind(edit_split_balance)
        .bind(edit_grain_amount)
        .bind(edit_grain_size)
        .bind(edit_crop_x)
        .bind(edit_crop_y)
        .bind(edit_crop_width)
        .bind(edit_crop_height)
        .bind(edit_crop_rotation)
        .bind(edit_crop_angle)
        .bind(edit_crop_flip_h)
        .bind(edit_crop_flip_v)
        .bind(&id)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InvalidOperation(format!("Failed to update photo: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(DomainError::PhotoNotFound);
        }

        Ok(())
    }

    async fn delete(&self, id: &PhotoId) -> DomainResult<()> {
        let id_str = id.to_string();

        let result = sqlx::query("DELETE FROM photos WHERE id = ?")
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::InvalidOperation(format!("Failed to delete photo: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(DomainError::PhotoNotFound);
        }

        Ok(())
    }

    async fn exists(&self, id: &PhotoId) -> DomainResult<bool> {
        let id_str = id.to_string();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM photos WHERE id = ?")
            .bind(&id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to check existence: {}", e))
            })?;

        Ok(count > 0)
    }

    async fn find_by_content_hash(&self, hash: &str) -> DomainResult<Option<Photo>> {
        let row = sqlx::query("SELECT * FROM photos WHERE content_hash = ? LIMIT 1")
            .bind(hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                DomainError::InvalidOperation(format!("Failed to find photo by hash: {}", e))
            })?;

        match row {
            Some(r) => Ok(Some(Self::row_to_photo(&r)?)),
            None => Ok(None),
        }
    }
}
