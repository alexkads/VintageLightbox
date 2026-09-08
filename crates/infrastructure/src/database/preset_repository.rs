use async_trait::async_trait;
use domain::entities::{preset::PresetAdjustments, Preset, PresetId};
use domain::repositories::PresetRepository;
use domain::{DomainError, DomainResult};
use sqlx::{Row, SqlitePool};
use std::str::FromStr;
use uuid::Uuid;

/// As colunas que toda leitura precisa. Uma constante porque eram três consultas
/// repetindo dezoito nomes, e acrescentar coluna significava lembrar das três.
const CAMPOS: &str = "id, name, is_system, adjustments";

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
        let ajustes = serde_json::to_string(&preset.adjustments)
            .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO presets (id, name, is_system, adjustments)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                is_system = excluded.is_system,
                adjustments = excluded.adjustments,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(id_str)
        .bind(&preset.name)
        .bind(preset.is_system)
        .bind(ajustes)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        Ok(())
    }

    async fn find_by_id(&self, id: &PresetId) -> DomainResult<Option<Preset>> {
        let id_str = id.to_string();

        let row = sqlx::query(&format!("SELECT {CAMPOS} FROM presets WHERE id = ?"))
            .bind(id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        row.map(|row| map_row_to_preset(&row)).transpose()
    }

    async fn find_all(&self) -> DomainResult<Vec<Preset>> {
        let rows = sqlx::query(&format!(
            "SELECT {CAMPOS} FROM presets ORDER BY name COLLATE NOCASE ASC"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        rows.iter().map(map_row_to_preset).collect()
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

    // ⚠️ **JSON quebrado vira predefinição vazia, e não erro.** Uma linha
    // ilegível derrubaria a listagem inteira — o fotógrafo perderia a lista por
    // causa de uma predefinição. Vazia ela aparece com "0" ao lado do nome, que
    // é visível e não destrói nada.
    let bruto: String = row.try_get("adjustments").unwrap_or_default();
    let adjustments: PresetAdjustments = serde_json::from_str(&bruto).unwrap_or_default();

    Ok(Preset {
        id,
        name,
        adjustments,
        is_system,
    })
}
