use async_trait::async_trait;
use domain::entities::{preset::PresetAdjustments, Preset, PresetId};
use domain::repositories::PresetRepository;
use domain::{DomainError, DomainResult};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone)]
pub struct SqlitePresetRepository {
    pool: SqlitePool,
}

impl SqlitePresetRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PresetRepository for SqlitePresetRepository {
    async fn save(&self, preset: &Preset) -> DomainResult<()> {
        let id_str = preset.id.to_string();

        sqlx::query(
            r#"
            INSERT INTO presets (
                id, name, is_system,
                exposure, contrast, temperature, tint,
                highlights, shadows, whites, blacks,
                clarity, vibrance, saturation,
                tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                is_system = excluded.is_system,
                exposure = excluded.exposure,
                contrast = excluded.contrast,
                temperature = excluded.temperature,
                tint = excluded.tint,
                highlights = excluded.highlights,
                shadows = excluded.shadows,
                whites = excluded.whites,
                blacks = excluded.blacks,
                clarity = excluded.clarity,
                vibrance = excluded.vibrance,
                saturation = excluded.saturation,
                tone_curve_shadows = excluded.tone_curve_shadows,
                tone_curve_darks = excluded.tone_curve_darks,
                tone_curve_lights = excluded.tone_curve_lights,
                tone_curve_highlights = excluded.tone_curve_highlights,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(id_str)
        .bind(&preset.name)
        .bind(preset.is_system)
        .bind(preset.adjustments.exposure)
        .bind(preset.adjustments.contrast)
        .bind(preset.adjustments.temperature)
        .bind(preset.adjustments.tint)
        .bind(preset.adjustments.highlights)
        .bind(preset.adjustments.shadows)
        .bind(preset.adjustments.whites)
        .bind(preset.adjustments.blacks)
        .bind(preset.adjustments.clarity)
        .bind(preset.adjustments.vibrance)
        .bind(preset.adjustments.saturation)
        .bind(preset.adjustments.tone_curve_shadows)
        .bind(preset.adjustments.tone_curve_darks)
        .bind(preset.adjustments.tone_curve_lights)
        .bind(preset.adjustments.tone_curve_highlights)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &PresetId) -> DomainResult<Option<Preset>> {
        let id_str = id.to_string();

        let row = sqlx::query(
            r#"
            SELECT
                id, name, is_system,
                exposure, contrast, temperature, tint,
                highlights, shadows, whites, blacks,
                clarity, vibrance, saturation,
                tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights
            FROM presets
            WHERE id = ?
            "#,
        )
        .bind(id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        if let Some(row) = row {
            Ok(Some(map_row_to_preset(&row)?))
        } else {
            Ok(None)
        }
    }

    async fn find_all(&self) -> DomainResult<Vec<Preset>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, name, is_system,
                exposure, contrast, temperature, tint,
                highlights, shadows, whites, blacks,
                clarity, vibrance, saturation,
                tone_curve_shadows, tone_curve_darks, tone_curve_lights, tone_curve_highlights
            FROM presets
            ORDER BY name ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        let mut presets = Vec::new();
        for row in rows {
            presets.push(map_row_to_preset(&row)?);
        }

        Ok(presets)
    }

    async fn delete(&self, id: &PresetId) -> DomainResult<()> {
        let id_str = id.to_string();
        sqlx::query("DELETE FROM presets WHERE id = ?")
            .bind(id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
        Ok(())
    }
}

fn map_row_to_preset(row: &sqlx::sqlite::SqliteRow) -> DomainResult<Preset> {
    let id_str: String = row
        .try_get("id")
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
    let uuid =
        Uuid::from_str(&id_str).map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
    let id = PresetId::from(uuid);

    let name: String = row
        .try_get("name")
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
    let is_system: bool = row
        .try_get("is_system")
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

    let adjustments = PresetAdjustments {
        exposure: row.try_get("exposure").ok(),
        contrast: row.try_get("contrast").ok(),
        temperature: row.try_get("temperature").ok(),
        tint: row.try_get("tint").ok(),
        highlights: row.try_get("highlights").ok(),
        shadows: row.try_get("shadows").ok(),
        whites: row.try_get("whites").ok(),
        blacks: row.try_get("blacks").ok(),
        clarity: row.try_get("clarity").ok(),
        vibrance: row.try_get("vibrance").ok(),
        saturation: row.try_get("saturation").ok(),
        tone_curve_shadows: row.try_get("tone_curve_shadows").ok(),
        tone_curve_darks: row.try_get("tone_curve_darks").ok(),
        tone_curve_lights: row.try_get("tone_curve_lights").ok(),
        tone_curve_highlights: row.try_get("tone_curve_highlights").ok(),
    };

    Ok(Preset {
        id,
        name,
        adjustments,
        is_system,
    })
}
