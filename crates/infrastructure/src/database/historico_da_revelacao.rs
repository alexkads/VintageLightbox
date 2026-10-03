//! O histórico da Revelação, por foto — a tabela `historico_da_revelacao`
//! (migration 028). O JSON é do ui-gpui (`revelacao/historico.rs`); aqui ele é
//! só texto.

use sqlx::{Row, SqlitePool};

#[derive(Clone)]
pub struct CatalogoDoHistorico {
    pool: SqlitePool,
}

fn erro(e: sqlx::Error) -> String {
    format!("o catálogo do histórico recusou: {e}")
}

impl CatalogoDoHistorico {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// O histórico gravado da foto. `None` quando ela nunca foi revelada aqui.
    pub async fn ler(&self, foto_id: &str) -> Result<Option<String>, String> {
        let linha = sqlx::query("SELECT json FROM historico_da_revelacao WHERE foto_id = ?")
            .bind(foto_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(erro)?;
        Ok(linha.map(|l| l.get::<String, _>("json")))
    }

    /// Grava o histórico da foto — **só se `versao` for maior** que a gravada.
    ///
    /// 🚨 Cada gesto grava numa tarefa do tokio, e duas tarefas terminam na
    /// ordem que quiserem: sem a versão, a que chegasse por último podia ser a
    /// mais velha, e o painel reabriria sem o último passo.
    pub async fn gravar(&self, foto_id: &str, json: &str, versao: i64) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO historico_da_revelacao (foto_id, json, versao) VALUES (?, ?, ?) \
             ON CONFLICT(foto_id) DO UPDATE SET json = excluded.json, \
             versao = excluded.versao, atualizado_em = CURRENT_TIMESTAMP \
             WHERE excluded.versao > historico_da_revelacao.versao",
        )
        .bind(foto_id)
        .bind(json)
        .bind(versao)
        .execute(&self.pool)
        .await
        .map_err(erro)?;
        Ok(())
    }

    /// Apaga o histórico da foto.
    pub async fn apagar(&self, foto_id: &str) -> Result<(), String> {
        sqlx::query("DELETE FROM historico_da_revelacao WHERE foto_id = ?")
            .bind(foto_id)
            .execute(&self.pool)
            .await
            .map_err(erro)?;
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::database::{create_pool, run_migrations};

    async fn catalogo() -> CatalogoDoHistorico {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();
        CatalogoDoHistorico::new(pool)
    }

    #[tokio::test]
    async fn grava_le_e_apaga() {
        let c = catalogo().await;
        assert_eq!(c.ler("foto-1").await.unwrap(), None);

        c.gravar("foto-1", "{\"a\":1}", 1).await.unwrap();
        c.gravar("site:abc", "{\"b\":2}", 2).await.unwrap();
        assert_eq!(c.ler("foto-1").await.unwrap().as_deref(), Some("{\"a\":1}"));
        assert_eq!(
            c.ler("site:abc").await.unwrap().as_deref(),
            Some("{\"b\":2}")
        );

        c.apagar("foto-1").await.unwrap();
        assert_eq!(c.ler("foto-1").await.unwrap(), None);
    }

    /// 🚨 A gravação mais velha que chega por último não passa por cima.
    #[tokio::test]
    async fn versao_menor_nao_sobrescreve() {
        let c = catalogo().await;
        c.gravar("foto-1", "novo", 10).await.unwrap();
        c.gravar("foto-1", "velho", 9).await.unwrap();
        assert_eq!(c.ler("foto-1").await.unwrap().as_deref(), Some("novo"));

        c.gravar("foto-1", "mais novo", 11).await.unwrap();
        assert_eq!(c.ler("foto-1").await.unwrap().as_deref(), Some("mais novo"));
    }
}
