//! SQLite Unit of Work Implementation
//!
//! Implementação do padrão Unit of Work para SQLite usando SQLx.
//!
//! ## Características
//!
//! - Transações atômicas com commit/rollback
//! - Rollback automático via RAII (Drop)
//! - Suporte a savepoints para rollback parcial
//! - Thread-safe e async-ready
//!
//! ## Exemplo de Uso
//!
//! ```rust,ignore
//! let uow = SqliteUnitOfWork::new(pool.clone());
//! let mut tx = uow.begin().await?;
//!
//! // Operações dentro da transação
//! sqlx::query("INSERT INTO photos (id, path) VALUES (?, ?)")
//!     .bind(&photo_id)
//!     .bind(&path)
//!     .execute(tx.connection())
//!     .await?;
//!
//! tx.commit().await?;  // Confirma todas as operações
//! ```

use async_trait::async_trait;
use domain::{
    ports::{TransactionResult, TransactionScope, TransactionState, UnitOfWork},
    DomainError, DomainResult,
};
use sqlx::{SqlitePool, Transaction, Sqlite};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Implementação do UnitOfWork para SQLite.
///
/// Gerencia transações de banco de dados garantindo
/// atomicidade e consistência das operações.
#[derive(Debug, Clone)]
pub struct SqliteUnitOfWork {
    pool: SqlitePool,
    has_active_transaction: Arc<AtomicBool>,
}

impl SqliteUnitOfWork {
    /// Cria uma nova instância do UnitOfWork.
    ///
    /// ## Parâmetros
    /// - `pool`: Pool de conexões SQLite
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            has_active_transaction: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Retorna o pool de conexões subjacente.
    ///
    /// Útil para operações que não precisam de transação.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

#[async_trait]
impl UnitOfWork for SqliteUnitOfWork {
    type Transaction = SqliteTransactionScope;

    async fn begin(&self) -> DomainResult<Self::Transaction> {
        // Verificar se já existe transação ativa
        if self.has_active_transaction.load(Ordering::SeqCst) {
            return Err(DomainError::InvalidOperation(
                "Transaction already active. Use savepoint for nested transactions.".to_string()
            ));
        }

        // Iniciar transação
        let tx = self.pool
            .begin()
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to begin transaction: {}", e)))?;

        self.has_active_transaction.store(true, Ordering::SeqCst);

        Ok(SqliteTransactionScope::new(
            tx,
            self.has_active_transaction.clone(),
        ))
    }

    fn has_active_transaction(&self) -> bool {
        self.has_active_transaction.load(Ordering::SeqCst)
    }
}

/// Escopo de uma transação SQLite ativa.
///
/// Esta struct mantém a transação aberta e garante rollback
/// automático se não houver commit explícito (padrão RAII).
///
/// ## Rollback Automático
///
/// Se `SqliteTransactionScope` for dropado sem `commit()` ser chamado,
/// a transação será automaticamente revertida. Isso garante segurança
/// mesmo em caso de panic ou early return.
pub struct SqliteTransactionScope {
    /// A transação SQLx subjacente
    /// Option para permitir take() no commit/rollback
    transaction: Option<Transaction<'static, Sqlite>>,
    /// Estado atual da transação
    state: TransactionState,
    /// Contador de operações
    operations_count: AtomicUsize,
    /// Referência ao flag de transação ativa do UnitOfWork
    active_flag: Arc<AtomicBool>,
    /// Lista de savepoints ativos
    savepoints: Vec<String>,
}

impl SqliteTransactionScope {
    /// Cria um novo escopo de transação.
    fn new(
        transaction: Transaction<'static, Sqlite>,
        active_flag: Arc<AtomicBool>,
    ) -> Self {
        Self {
            transaction: Some(transaction),
            state: TransactionState::Active,
            operations_count: AtomicUsize::new(0),
            active_flag,
            savepoints: Vec::new(),
        }
    }

    /// Retorna uma referência mutável à conexão da transação.
    ///
    /// Use isso para executar queries dentro da transação.
    ///
    /// ## Exemplo
    /// ```rust,ignore
    /// sqlx::query("UPDATE photos SET edited = true WHERE id = ?")
    ///     .bind(&photo_id)
    ///     .execute(tx.connection())
    ///     .await?;
    /// ```
    pub fn connection(&mut self) -> &mut Transaction<'static, Sqlite> {
        self.transaction.as_mut().expect("Transaction already consumed")
    }

    /// Incrementa o contador de operações.
    ///
    /// Chamado automaticamente pelos repositories transacionais.
    pub fn record_operation(&self) {
        self.operations_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Verifica se a transação ainda está ativa.
    pub fn is_active(&self) -> bool {
        self.state == TransactionState::Active && self.transaction.is_some()
    }
}

impl std::fmt::Debug for SqliteTransactionScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteTransactionScope")
            .field("state", &self.state)
            .field("operations_count", &self.operations_count.load(Ordering::SeqCst))
            .field("savepoints", &self.savepoints)
            .field("has_transaction", &self.transaction.is_some())
            .finish()
    }
}

#[async_trait]
impl TransactionScope for SqliteTransactionScope {
    async fn commit(mut self) -> DomainResult<TransactionResult> {
        if self.state != TransactionState::Active {
            return Err(DomainError::InvalidOperation(format!(
                "Cannot commit transaction in state {:?}",
                self.state
            )));
        }

        let tx = self.transaction.take().ok_or_else(|| {
            DomainError::InvalidOperation("Transaction already consumed".to_string())
        })?;

        tx.commit()
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to commit: {}", e)))?;

        self.state = TransactionState::Committed;
        self.active_flag.store(false, Ordering::SeqCst);

        Ok(TransactionResult {
            state: TransactionState::Committed,
            operations_count: self.operations_count.load(Ordering::SeqCst),
        })
    }

    async fn rollback(mut self) -> DomainResult<TransactionResult> {
        if self.state != TransactionState::Active {
            return Err(DomainError::InvalidOperation(format!(
                "Cannot rollback transaction in state {:?}",
                self.state
            )));
        }

        let tx = self.transaction.take().ok_or_else(|| {
            DomainError::InvalidOperation("Transaction already consumed".to_string())
        })?;

        tx.rollback()
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to rollback: {}", e)))?;

        self.state = TransactionState::RolledBack;
        self.active_flag.store(false, Ordering::SeqCst);

        Ok(TransactionResult {
            state: TransactionState::RolledBack,
            operations_count: self.operations_count.load(Ordering::SeqCst),
        })
    }

    fn state(&self) -> TransactionState {
        self.state
    }

    fn operations_count(&self) -> usize {
        self.operations_count.load(Ordering::SeqCst)
    }

    async fn savepoint(&mut self, name: &str) -> DomainResult<()> {
        if !self.is_active() {
            return Err(DomainError::InvalidOperation(
                "Cannot create savepoint: transaction not active".to_string()
            ));
        }

        let tx = self.connection();
        
        // SQLite suporta savepoints
        sqlx::query(&format!("SAVEPOINT {}", name))
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to create savepoint: {}", e)))?;

        self.savepoints.push(name.to_string());
        Ok(())
    }

    async fn rollback_to_savepoint(&mut self, name: &str) -> DomainResult<()> {
        if !self.is_active() {
            return Err(DomainError::InvalidOperation(
                "Cannot rollback to savepoint: transaction not active".to_string()
            ));
        }

        if !self.savepoints.contains(&name.to_string()) {
            return Err(DomainError::InvalidOperation(format!(
                "Savepoint '{}' not found",
                name
            )));
        }

        let tx = self.connection();
        
        sqlx::query(&format!("ROLLBACK TO SAVEPOINT {}", name))
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to rollback to savepoint: {}", e)))?;

        // Remove savepoints criados após este
        while let Some(sp) = self.savepoints.pop() {
            if sp == name {
                self.savepoints.push(sp); // Mantém o savepoint atual
                break;
            }
        }

        Ok(())
    }

    async fn release_savepoint(&mut self, name: &str) -> DomainResult<()> {
        if !self.is_active() {
            return Err(DomainError::InvalidOperation(
                "Cannot release savepoint: transaction not active".to_string()
            ));
        }

        if !self.savepoints.contains(&name.to_string()) {
            return Err(DomainError::InvalidOperation(format!(
                "Savepoint '{}' not found",
                name
            )));
        }

        let tx = self.connection();
        
        sqlx::query(&format!("RELEASE SAVEPOINT {}", name))
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::InfrastructureError(format!("Failed to release savepoint: {}", e)))?;

        // Remove o savepoint da lista
        self.savepoints.retain(|sp| sp != name);

        Ok(())
    }
}

/// Implementação do Drop para garantir rollback automático.
///
/// Se a transação for dropada sem commit, fará rollback.
/// Isso é crucial para segurança em caso de panic ou early return.
impl Drop for SqliteTransactionScope {
    fn drop(&mut self) {
        if self.state == TransactionState::Active {
            // Se ainda está ativa e será dropada, significa que não houve commit
            // O SQLx fará rollback automaticamente quando a Transaction for dropada
            self.active_flag.store(false, Ordering::SeqCst);
            
            // Log para debugging
            #[cfg(debug_assertions)]
            {
                let ops = self.operations_count.load(Ordering::SeqCst);
                if ops > 0 {
                    eprintln!(
                        "Warning: Transaction with {} operations dropped without commit. Rolling back.",
                        ops
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn create_test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("Failed to create test pool")
    }

    async fn setup_test_table(pool: &SqlitePool) {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS test_items (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL
            )"
        )
        .execute(pool)
        .await
        .expect("Failed to create test table");
    }

    #[tokio::test]
    async fn test_begin_creates_active_transaction() {
        let pool = create_test_pool().await;
        let uow = SqliteUnitOfWork::new(pool);

        let tx = uow.begin().await.expect("Should begin transaction");
        
        assert_eq!(tx.state(), TransactionState::Active);
        assert!(uow.has_active_transaction());
    }

    #[tokio::test]
    async fn test_commit_changes_state() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool);

        let tx = uow.begin().await.unwrap();
        let result = tx.commit().await.unwrap();

        assert_eq!(result.state, TransactionState::Committed);
        assert!(!uow.has_active_transaction());
    }

    #[tokio::test]
    async fn test_rollback_changes_state() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool);

        let tx = uow.begin().await.unwrap();
        let result = tx.rollback().await.unwrap();

        assert_eq!(result.state, TransactionState::RolledBack);
        assert!(!uow.has_active_transaction());
    }

    #[tokio::test]
    async fn test_commit_persists_data() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool.clone());

        let mut tx = uow.begin().await.unwrap();
        
        sqlx::query("INSERT INTO test_items (id, name) VALUES ('1', 'test')")
            .execute(&mut **tx.connection())
            .await
            .unwrap();
        tx.record_operation();
        
        tx.commit().await.unwrap();

        // Verificar que os dados persistiram
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM test_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn test_rollback_discards_data() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool.clone());

        let mut tx = uow.begin().await.unwrap();
        
        sqlx::query("INSERT INTO test_items (id, name) VALUES ('1', 'test')")
            .execute(&mut **tx.connection())
            .await
            .unwrap();
        tx.record_operation();
        
        tx.rollback().await.unwrap();

        // Verificar que os dados NÃO persistiram
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM test_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn test_drop_without_commit_rolls_back() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool.clone());

        {
            let mut tx = uow.begin().await.unwrap();
            
            sqlx::query("INSERT INTO test_items (id, name) VALUES ('1', 'test')")
                .execute(&mut **tx.connection())
                .await
                .unwrap();
            
            // tx é dropado aqui sem commit
        }

        // Verificar que os dados NÃO persistiram (rollback automático)
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM test_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(count.0, 0);
        assert!(!uow.has_active_transaction());
    }

    #[tokio::test]
    async fn test_savepoint_and_rollback_to_savepoint() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool.clone());

        let mut tx = uow.begin().await.unwrap();
        
        // Inserir primeiro item
        sqlx::query("INSERT INTO test_items (id, name) VALUES ('1', 'first')")
            .execute(&mut **tx.connection())
            .await
            .unwrap();
        
        // Criar savepoint
        tx.savepoint("sp1").await.unwrap();
        
        // Inserir segundo item
        sqlx::query("INSERT INTO test_items (id, name) VALUES ('2', 'second')")
            .execute(&mut **tx.connection())
            .await
            .unwrap();
        
        // Rollback para savepoint (desfaz segundo item)
        tx.rollback_to_savepoint("sp1").await.unwrap();
        
        // Commit
        tx.commit().await.unwrap();

        // Verificar que apenas o primeiro item existe
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM test_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(count.0, 1);
        
        let name: (String,) = sqlx::query_as("SELECT name FROM test_items WHERE id = '1'")
            .fetch_one(&pool)
            .await
            .unwrap();
        
        assert_eq!(name.0, "first");
    }

    #[tokio::test]
    async fn test_operations_count() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool);

        let tx = uow.begin().await.unwrap();
        assert_eq!(tx.operations_count(), 0);
        
        tx.record_operation();
        tx.record_operation();
        tx.record_operation();
        
        assert_eq!(tx.operations_count(), 3);
        
        let result = tx.commit().await.unwrap();
        assert_eq!(result.operations_count, 3);
    }

    #[tokio::test]
    async fn test_cannot_begin_with_active_transaction() {
        let pool = create_test_pool().await;
        let uow = SqliteUnitOfWork::new(pool);

        let _tx1 = uow.begin().await.unwrap();
        
        // Tentar iniciar outra transação deve falhar
        let result = uow.begin().await;
        
        assert!(result.is_err());
        match result {
            Err(DomainError::InvalidOperation(msg)) => {
                assert!(msg.contains("already active"));
            }
            _ => panic!("Expected InvalidOperation error"),
        }
    }

    #[tokio::test]
    async fn test_cannot_commit_twice() {
        let pool = create_test_pool().await;
        setup_test_table(&pool).await;
        let uow = SqliteUnitOfWork::new(pool);

        let tx = uow.begin().await.unwrap();
        tx.commit().await.unwrap();
        
        // A transação foi consumida, não pode ser usada novamente
        // (Rust garante isso em tempo de compilação com ownership)
    }
}
