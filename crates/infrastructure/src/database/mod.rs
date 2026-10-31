//! Database module
//!
//! Implementações de repositories usando SQLite.

pub mod collection_repository;
pub mod desempenho;
pub mod edicoes;
pub mod espera;
pub mod fila_de_sincronizacao;
pub mod historico_da_revelacao;
pub mod photo_repository;
pub mod preset_repository;
pub mod revelacoes_do_site;

pub use collection_repository::CollectionRepositoryImpl;
pub use desempenho::SqliteDesempenho;
pub use edicoes::{CatalogoDeEdicoes, EdicaoRegistrada};
pub use fila_de_sincronizacao::{EstadoDoGesto, FilaDeSincronizacao, GestoGuardado, GestoNovo};
pub use historico_da_revelacao::CatalogoDoHistorico;
pub use photo_repository::PhotoRepositoryImpl;
pub use preset_repository::SqlitePresetRepository;
pub use revelacoes_do_site::SqliteRevelacoesDoSite;

use std::str::FromStr;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

/// Cria um pool de conexões SQLite
///
/// 🔒 **Em WAL desde 31/out/2026** (o "database is locked" de Gramado, ver
/// [`espera`]): no diário clássico quem lê segura o arquivo, e a releitura do
/// catálogo inteiro fazia a cópia e a revelação desistirem. `synchronous =
/// NORMAL` é o par do WAL — a foto gravada sobrevive ao app fechar; o que
/// arrisca é só o último gesto numa queda de energia.
pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opcoes = SqliteConnectOptions::from_str(database_url)?
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(espera::ESPERA_DO_SQLITE);
    SqlitePoolOptions::new()
        .max_connections(espera::CONEXOES)
        .acquire_timeout(espera::ESPERA_POR_CONEXAO)
        .connect_with(opcoes)
        .await
}

/// Executa migrations do banco de dados
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Usar migrate! macro embedda as migrations no binário e garante versionamento
    // O caminho é relativo a este arquivo (src/database/mod.rs) -> ../../migrations
    sqlx::migrate!("./migrations").run(pool).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_in_memory_pool() {
        let pool = create_pool("sqlite::memory:").await;
        assert!(pool.is_ok());
    }

    /// 🔒 **"database is locked" na importação do estúdio de Gramado
    /// (28–31/out/2026).** No diário clássico do SQLite quem lê segura o
    /// arquivo e quem grava espera; passados os 5 s do sqlx, a gravação da
    /// foto desistia ("6 fotos não foram copiadas"). Em WAL a leitura não
    /// bloqueia a escrita.
    #[tokio::test]
    async fn gravar_com_leitura_aberta_nao_trava() {
        let pasta = tempfile::tempdir().unwrap();
        let url = format!("sqlite:{}?mode=rwc", pasta.path().join("c.db").display());
        let pool = create_pool(&url).await.unwrap();
        sqlx::query("CREATE TABLE t (x INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO t VALUES (1)")
            .execute(&pool)
            .await
            .unwrap();

        // Uma leitura no meio, como a da grade enquanto a cópia anda.
        let mut leitura = pool.begin().await.unwrap();
        sqlx::query("SELECT x FROM t")
            .fetch_all(&mut *leitura)
            .await
            .unwrap();

        let gravacao = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            sqlx::query("INSERT INTO t VALUES (2)").execute(&pool),
        )
        .await;
        assert!(
            matches!(gravacao, Ok(Ok(_))),
            "a gravação esperou a leitura: {gravacao:?}"
        );
        leitura.rollback().await.unwrap();
    }

    /// Muitas gravações ao mesmo tempo, como as levas da importação somadas à
    /// fila de sincronização: nenhuma desiste.
    #[tokio::test]
    async fn gravacoes_simultaneas_entram_todas() {
        let pasta = tempfile::tempdir().unwrap();
        let url = format!("sqlite:{}?mode=rwc", pasta.path().join("c.db").display());
        let pool = create_pool(&url).await.unwrap();
        sqlx::query("CREATE TABLE t (x INTEGER)")
            .execute(&pool)
            .await
            .unwrap();
        let tarefas: Vec<_> = (0..200)
            .map(|i| {
                let pool = pool.clone();
                tokio::spawn(async move {
                    sqlx::query("INSERT INTO t VALUES (?)")
                        .bind(i)
                        .execute(&pool)
                        .await
                })
            })
            .collect();
        for t in tarefas {
            t.await.unwrap().unwrap();
        }
        let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM t")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(n, 200);
    }

    #[tokio::test]
    async fn test_run_migrations() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let result = run_migrations(&pool).await;
        assert!(result.is_ok(), "{result:?}");
    }
}
