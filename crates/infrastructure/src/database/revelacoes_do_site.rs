//! O depósito das revelações de fotos que só existem no site.
//!
//! 🔑 **É o mesmo SQLite do catálogo**, e não um banco novo: o pool que chega
//! aqui é o que abre `photos`, `presets` e as coleções. O que a tabela
//! `revelacoes_do_site` acrescenta é o lugar de guardar a receita de uma foto
//! que **não tem arquivo neste disco** — e por isso não é linha de `photos`,
//! cujas duas migrations do pós-venda (017 e 019) dizem, cada uma à sua
//! maneira, que ali mora "uma foto no disco".

use async_trait::async_trait;
use domain::repositories::RevelacoesDoSiteRepository;
use domain::{DomainError, DomainResult};
use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct SqliteRevelacoesDoSite {
    pool: SqlitePool,
}

impl SqliteRevelacoesDoSite {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RevelacoesDoSiteRepository for SqliteRevelacoesDoSite {
    async fn guardar(&self, foto_no_site: &str, ajustes: &str) -> DomainResult<()> {
        // O depósito guarda o **estado**, não a história: o gesto seguinte
        // substitui o anterior, como o autosave do site faz.
        sqlx::query(
            r#"
            INSERT INTO revelacoes_do_site (pos_venda_foto_id, ajustes, atualizada_em)
            VALUES (?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(pos_venda_foto_id) DO UPDATE SET
                ajustes = excluded.ajustes,
                atualizada_em = CURRENT_TIMESTAMP
            "#,
        )
        .bind(foto_no_site)
        .bind(ajustes)
        .execute(&self.pool)
        .await
        .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
        Ok(())
    }

    async fn todas(&self) -> DomainResult<Vec<(String, String)>> {
        let linhas = sqlx::query("SELECT pos_venda_foto_id, ajustes FROM revelacoes_do_site")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;

        Ok(linhas
            .into_iter()
            .map(|l| (l.get("pos_venda_foto_id"), l.get("ajustes")))
            .collect())
    }

    async fn esquecer(&self, foto_no_site: &str) -> DomainResult<()> {
        sqlx::query("DELETE FROM revelacoes_do_site WHERE pos_venda_foto_id = ?")
            .bind(foto_no_site)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::InfrastructureError(e.to_string()))?;
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    async fn deposito() -> SqliteRevelacoesDoSite {
        let pool = crate::database::create_pool("sqlite::memory:")
            .await
            .expect("abrir o banco em memória");
        crate::database::run_migrations(&pool)
            .await
            .expect("as migrations rodam");
        SqliteRevelacoesDoSite::new(pool)
    }

    /// 🔑 O ciclo do depósito, contra SQLite de verdade — inclusive a migration.
    #[tokio::test]
    async fn guarda_substitui_e_esquece() {
        let deposito = deposito().await;

        deposito
            .guardar("remota-1", r#"{"exposure":1.25}"#)
            .await
            .unwrap();
        deposito
            .guardar("remota-2", r#"{"contrast":1.1}"#)
            .await
            .unwrap();
        let mut todas = deposito.todas().await.unwrap();
        todas.sort();
        assert_eq!(
            todas,
            vec![
                ("remota-1".to_string(), r#"{"exposure":1.25}"#.to_string()),
                ("remota-2".to_string(), r#"{"contrast":1.1}"#.to_string()),
            ]
        );

        // 🚨 O segundo gesto na mesma foto **substitui**. Sem o `ON CONFLICT` o
        // insert falharia na chave primária e o autosave morreria calado no
        // segundo arrasto — o defeito que esta tabela existe para não ter.
        deposito
            .guardar("remota-1", r#"{"exposure":0.5}"#)
            .await
            .unwrap();
        assert_eq!(deposito.todas().await.unwrap().len(), 2);

        deposito.esquecer("remota-1").await.unwrap();
        assert_eq!(
            deposito.todas().await.unwrap(),
            vec![("remota-2".to_string(), r#"{"contrast":1.1}"#.to_string())],
            "a que subiu sai, e só ela"
        );

        // Esquecer o que não está lá não é erro: o "Salvar na galeria" de uma
        // foto que nunca teve gesto nenhum passa por aqui.
        deposito.esquecer("nunca-existiu").await.unwrap();
    }
}
