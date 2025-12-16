//! Database module
//!
//! Implementações de repositories usando SQLite.

pub mod photo_repository;

pub use photo_repository::PhotoRepositoryImpl;

use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

/// Cria um pool de conexões SQLite
pub async fn create_pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
}

/// Executa migrations do banco de dados
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // Ler e executar migration SQL
    let migration_sql = include_str!("../../migrations/001_initial_schema.sql");
    
    sqlx::raw_sql(migration_sql)
        .execute(pool)
        .await?;
    
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

    #[tokio::test]
    async fn test_run_migrations() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let result = run_migrations(&pool).await;
        assert!(result.is_ok());
    }
}
