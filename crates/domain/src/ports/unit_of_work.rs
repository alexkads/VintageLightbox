//! Unit of Work Pattern
//!
//! Define o contrato para gerenciamento de transações de banco de dados
//! seguindo o padrão Unit of Work de Martin Fowler.
//!
//! ## Problema que resolve
//!
//! Sem Unit of Work, operações de banco são isoladas:
//! ```rust,ignore
//! repository.save(&photo).await?;    // Commit implícito
//! repository.update(&photo2).await?; // Commit implícito - se falhar, photo já foi salva!
//! ```
//!
//! Com Unit of Work, operações são atômicas:
//! ```rust,ignore
//! let mut uow = unit_of_work.begin().await?;
//! repository.save_within(&mut uow, &photo).await?;
//! repository.update_within(&mut uow, &photo2).await?;
//! uow.commit().await?;  // Ambas ou nenhuma
//! ```
//!
//! ## Garantias do Contrato
//!
//! 1. **Atomicidade**: Todas as operações dentro de uma transação são commitadas juntas ou revertidas juntas
//! 2. **Isolamento**: Transações não veem mudanças de outras transações não commitadas
//! 3. **Rollback automático**: Se `TransactionScope` é dropado sem commit, faz rollback
//! 4. **Composabilidade**: Múltiplos repositories podem participar da mesma transação

use crate::DomainResult;
use async_trait::async_trait;
use std::fmt::Debug;

/// Estado de uma transação
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Transação ativa, operações podem ser executadas
    Active,
    /// Transação foi commitada com sucesso
    Committed,
    /// Transação foi revertida (rollback)
    RolledBack,
}

/// Resultado de uma operação de transação
#[derive(Debug, Clone)]
pub struct TransactionResult {
    /// Estado final da transação
    pub state: TransactionState,
    /// Número de operações executadas
    pub operations_count: usize,
}

/// Contrato para gerenciamento de transações.
///
/// Implementações devem garantir:
/// - Rollback automático se a transação não for commitada
/// - Thread-safety para uso em ambientes async
/// - Suporte a nested transactions (savepoints) quando possível
#[async_trait]
pub trait UnitOfWork: Send + Sync + Debug {
    /// Tipo do escopo de transação retornado por begin()
    type Transaction: TransactionScope;

    /// Inicia uma nova transação.
    ///
    /// ## Retorno
    /// - `Ok(Transaction)`: Escopo de transação ativo
    /// - `Err`: Falha ao iniciar transação
    ///
    /// ## Exemplo
    /// ```rust,ignore
    /// let uow = unit_of_work.begin().await?;
    /// // ... operações ...
    /// uow.commit().await?;
    /// ```
    async fn begin(&self) -> DomainResult<Self::Transaction>;

    /// Verifica se há uma transação ativa.
    fn has_active_transaction(&self) -> bool;
}

/// Escopo de uma transação ativa.
///
/// Este trait representa uma transação em andamento.
/// Quando dropado sem commit, deve fazer rollback automaticamente (RAII).
///
/// ## Implementação
///
/// O padrão RAII garante que mesmo em caso de panic ou early return,
/// a transação será revertida se não houver commit explícito.
#[async_trait]
pub trait TransactionScope: Send + Debug {
    /// Confirma todas as operações da transação.
    ///
    /// Após commit, a transação não pode mais ser usada.
    ///
    /// ## Retorno
    /// - `Ok(TransactionResult)`: Commit bem-sucedido
    /// - `Err`: Falha no commit (transação será revertida)
    async fn commit(self) -> DomainResult<TransactionResult>;

    /// Reverte todas as operações da transação.
    ///
    /// Pode ser chamado explicitamente ou acontece automaticamente no drop.
    ///
    /// ## Retorno
    /// - `Ok(TransactionResult)`: Rollback bem-sucedido
    /// - `Err`: Falha no rollback (situação crítica)
    async fn rollback(self) -> DomainResult<TransactionResult>;

    /// Retorna o estado atual da transação.
    fn state(&self) -> TransactionState;

    /// Retorna o número de operações registradas na transação.
    fn operations_count(&self) -> usize;

    /// Cria um savepoint dentro da transação atual.
    ///
    /// Savepoints permitem rollback parcial sem afetar toda a transação.
    ///
    /// ## Parâmetros
    /// - `name`: Nome único do savepoint
    ///
    /// ## Nota
    /// Nem todos os bancos de dados suportam savepoints.
    async fn savepoint(&mut self, name: &str) -> DomainResult<()>;

    /// Reverte para um savepoint específico.
    ///
    /// Desfaz operações desde o savepoint, mas mantém a transação ativa.
    async fn rollback_to_savepoint(&mut self, name: &str) -> DomainResult<()>;

    /// Libera um savepoint sem reverter.
    ///
    /// As operações desde o savepoint são mantidas.
    async fn release_savepoint(&mut self, name: &str) -> DomainResult<()>;
}

/// Trait auxiliar para repositories que suportam operações transacionais.
///
/// Repositories que implementam este trait podem participar de transações
/// gerenciadas pelo UnitOfWork.
///
/// ## Type Parameters
/// - `T`: O tipo de TransactionScope usado
/// - `E`: O tipo de entidade que o repository gerencia
/// - `Id`: O tipo do identificador da entidade
#[async_trait]
pub trait TransactionalRepository<T, E, Id>: Send + Sync
where
    T: TransactionScope,
    E: Send + Sync,
    Id: Send + Sync,
{
    /// Salva uma entidade dentro de uma transação existente.
    async fn save_in_transaction<'a>(
        &self,
        tx: &'a mut T,
        entity: &E,
    ) -> DomainResult<()>;

    /// Atualiza uma entidade dentro de uma transação existente.
    async fn update_in_transaction<'a>(
        &self,
        tx: &'a mut T,
        entity: &E,
    ) -> DomainResult<()>;

    /// Remove uma entidade dentro de uma transação existente.
    async fn delete_in_transaction<'a>(
        &self,
        tx: &'a mut T,
        id: &Id,
    ) -> DomainResult<()>;
}

/// Helper para executar operações em uma transação com rollback automático em caso de erro.
///
/// Este helper simplifica o uso comum do UnitOfWork:
///
/// ```rust,ignore
/// let result = execute_in_transaction(&uow, |tx| async move {
///     repo.save_in_transaction(&mut tx, &photo).await?;
///     repo.update_in_transaction(&mut tx, &photo2).await?;
///     Ok(())
/// }).await?;
/// ```
pub async fn execute_in_transaction<U, F, Fut, R>(
    uow: &U,
    operation: F,
) -> DomainResult<R>
where
    U: UnitOfWork,
    F: FnOnce(U::Transaction) -> Fut + Send,
    Fut: std::future::Future<Output = DomainResult<(U::Transaction, R)>> + Send,
    R: Send,
{
    let tx = uow.begin().await?;
    
    match operation(tx).await {
        Ok((tx, result)) => {
            tx.commit().await?;
            Ok(result)
        }
        Err(e) => {
            // Transaction será dropada e fará rollback automaticamente
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_state_variants() {
        assert_eq!(TransactionState::Active, TransactionState::Active);
        assert_ne!(TransactionState::Active, TransactionState::Committed);
        assert_ne!(TransactionState::Committed, TransactionState::RolledBack);
    }

    #[test]
    fn test_transaction_result_creation() {
        let result = TransactionResult {
            state: TransactionState::Committed,
            operations_count: 5,
        };
        
        assert_eq!(result.state, TransactionState::Committed);
        assert_eq!(result.operations_count, 5);
    }
}
