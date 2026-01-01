//! Testes E2E para integração UnitOfWork
//!
//! Verifica que operações de edição são atômicas e permitem rollback.
//! Usa color_label como campo de teste por não ter valor default.

use domain::{
    entities::Photo,
    ports::{TransactionScope, UnitOfWork, TransactionState},
    repositories::PhotoRepository,
    value_objects::{FilePath, PhotoId, ColorLabel},
};
use infrastructure::{
    database::{PhotoRepositoryImpl, SqliteUnitOfWork},
    create_pool, run_migrations,
};
use std::path::PathBuf;

/// Cria um pool de teste com banco em memória
async fn create_test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("Failed to create test pool");
    
    run_migrations(&pool)
        .await
        .expect("Failed to run migrations");
    
    pool
}

/// Cria uma foto de teste
fn create_test_photo(name: &str) -> Photo {
    let file_path = FilePath::new(PathBuf::from(format!("/test/photos/{}.jpg", name)))
        .expect("Valid test path");
    Photo::new(file_path)
}

#[tokio::test]
async fn test_commit_persists_color_label_change() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());
    let repo = PhotoRepositoryImpl::new(pool.clone());

    // Criar foto inicial
    let photo = create_test_photo("commit_test");
    let photo_id = photo.id();
    repo.save(&photo).await.expect("Failed to save initial photo");

    // Verificar estado inicial
    let initial = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(initial.color_label(), None);

    // Iniciar transação
    let mut tx = uow.begin().await.expect("Failed to begin transaction");

    // Atualizar color_label dentro da transação
    sqlx::query("UPDATE photos SET color_label = ? WHERE id = ?")
        .bind("red")
        .bind(photo_id.to_string())
        .execute(&mut **tx.connection())
        .await
        .expect("Failed to update in transaction");

    tx.record_operation();

    // Commit
    let result = tx.commit().await.expect("Failed to commit");
    assert_eq!(result.state, TransactionState::Committed);
    assert_eq!(result.operations_count, 1);

    // Verificar que a mudança persistiu
    let final_photo = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(final_photo.color_label(), Some(ColorLabel::Red));
}

#[tokio::test]
async fn test_rollback_discards_color_label_change() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());
    let repo = PhotoRepositoryImpl::new(pool.clone());

    // Criar foto inicial
    let photo = create_test_photo("rollback_test");
    let photo_id = photo.id();
    repo.save(&photo).await.expect("Failed to save initial photo");

    // Iniciar transação
    let mut tx = uow.begin().await.expect("Failed to begin transaction");

    // Atualizar color_label dentro da transação
    sqlx::query("UPDATE photos SET color_label = ? WHERE id = ?")
        .bind("green")
        .bind(photo_id.to_string())
        .execute(&mut **tx.connection())
        .await
        .expect("Failed to update in transaction");

    tx.record_operation();

    // Rollback ao invés de commit
    let result = tx.rollback().await.expect("Failed to rollback");
    assert_eq!(result.state, TransactionState::RolledBack);

    // Verificar que as mudanças NÃO persistiram
    let final_photo = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(final_photo.color_label(), None, "Color label should be unchanged after rollback");
}

#[tokio::test]
async fn test_drop_without_commit_rolls_back() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());
    let repo = PhotoRepositoryImpl::new(pool.clone());

    // Criar foto inicial
    let photo = create_test_photo("drop_test");
    let photo_id = photo.id();
    repo.save(&photo).await.expect("Failed to save initial photo");

    // Escopo onde transação é criada e dropada sem commit
    {
        let mut tx = uow.begin().await.expect("Failed to begin transaction");

        sqlx::query("UPDATE photos SET color_label = ? WHERE id = ?")
            .bind("blue")
            .bind(photo_id.to_string())
            .execute(&mut **tx.connection())
            .await
            .expect("Failed to update");

        // tx é dropado aqui sem commit - deve fazer rollback automático
    }

    // Verificar que rollback automático aconteceu
    let final_photo = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(final_photo.color_label(), None, "Color label should be None after implicit rollback");
}

#[tokio::test]
async fn test_savepoint_partial_rollback() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());
    let repo = PhotoRepositoryImpl::new(pool.clone());

    // Criar duas fotos
    let photo1 = create_test_photo("savepoint_1");
    let photo2 = create_test_photo("savepoint_2");
    let photo1_id = photo1.id();
    let photo2_id = photo2.id();
    repo.save(&photo1).await.unwrap();
    repo.save(&photo2).await.unwrap();

    let mut tx = uow.begin().await.unwrap();

    // Editar primeira foto
    sqlx::query("UPDATE photos SET color_label = ? WHERE id = ?")
        .bind("red")
        .bind(photo1_id.to_string())
        .execute(&mut **tx.connection())
        .await
        .unwrap();

    // Criar savepoint
    tx.savepoint("before_second_edit").await.unwrap();

    // Editar segunda foto
    sqlx::query("UPDATE photos SET color_label = ? WHERE id = ?")
        .bind("green")
        .bind(photo2_id.to_string())
        .execute(&mut **tx.connection())
        .await
        .unwrap();

    // Rollback para savepoint (desfaz edição da segunda foto)
    tx.rollback_to_savepoint("before_second_edit").await.unwrap();

    // Commit
    tx.commit().await.unwrap();

    // Verificar resultados
    let final_photo1 = repo.find_by_id(&photo1_id).await.unwrap().unwrap();
    let final_photo2 = repo.find_by_id(&photo2_id).await.unwrap().unwrap();

    assert_eq!(final_photo1.color_label(), Some(ColorLabel::Red), "Photo1 edit should be committed");
    assert_eq!(final_photo2.color_label(), None, "Photo2 edit should be rolled back to savepoint");
}

#[tokio::test]
async fn test_multiple_operations_in_transaction() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());
    let repo = PhotoRepositoryImpl::new(pool.clone());

    // Criar múltiplas fotos e guardar IDs
    let photos: Vec<(Photo, PhotoId)> = (0..3)
        .map(|i| {
            let photo = create_test_photo(&format!("multi_{}", i));
            let id = photo.id();
            (photo, id)
        })
        .collect();
    
    for (photo, _) in &photos {
        repo.save(photo).await.unwrap();
    }

    let colors = ["red", "green", "blue"];

    // Editar todas em uma transação
    let mut tx = uow.begin().await.unwrap();

    for (idx, (_, id)) in photos.iter().enumerate() {
        sqlx::query("UPDATE photos SET color_label = ? WHERE id = ?")
            .bind(colors[idx])
            .bind(id.to_string())
            .execute(&mut **tx.connection())
            .await
            .unwrap();
        tx.record_operation();
    }

    let result = tx.commit().await.unwrap();
    assert_eq!(result.operations_count, 3);

    // Verificar todas as edições
    let expected_colors = [ColorLabel::Red, ColorLabel::Green, ColorLabel::Blue];
    for (idx, (_, id)) in photos.iter().enumerate() {
        let loaded = repo.find_by_id(id).await.unwrap().unwrap();
        assert_eq!(loaded.color_label(), Some(expected_colors[idx]));
    }
}

#[tokio::test]
async fn test_transaction_error_triggers_rollback() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());
    let repo = PhotoRepositoryImpl::new(pool.clone());

    let photo = create_test_photo("error_test");
    let photo_id = photo.id();
    repo.save(&photo).await.unwrap();

    let mut tx = uow.begin().await.unwrap();

    // Primeira operação válida
    sqlx::query("UPDATE photos SET color_label = ? WHERE id = ?")
        .bind("yellow")
        .bind(photo_id.to_string())
        .execute(&mut **tx.connection())
        .await
        .unwrap();

    // Segunda operação que falha (coluna inexistente)
    let error_result = sqlx::query("UPDATE photos SET nonexistent_column = 'x' WHERE id = ?")
        .bind(photo_id.to_string())
        .execute(&mut **tx.connection())
        .await;

    assert!(error_result.is_err(), "Query with invalid column should fail");

    // Rollback explícito após erro
    tx.rollback().await.unwrap();

    // Verificar que nenhuma edição persistiu
    let final_photo = repo.find_by_id(&photo_id).await.unwrap().unwrap();
    assert_eq!(final_photo.color_label(), None, "All changes should be rolled back after error");
}

#[tokio::test]
async fn test_uow_can_begin_new_transaction_after_commit() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());

    // Primeira transação
    {
        let tx = uow.begin().await.unwrap();
        tx.commit().await.unwrap();
    }

    // Segunda transação deve funcionar
    {
        let tx = uow.begin().await.unwrap();
        tx.commit().await.unwrap();
    }

    assert!(!uow.has_active_transaction());
}

#[tokio::test]
async fn test_uow_can_begin_new_transaction_after_rollback() {
    let pool = create_test_pool().await;
    let uow = SqliteUnitOfWork::new(pool.clone());

    // Primeira transação com rollback
    {
        let tx = uow.begin().await.unwrap();
        tx.rollback().await.unwrap();
    }

    // Segunda transação deve funcionar
    {
        let tx = uow.begin().await.unwrap();
        tx.commit().await.unwrap();
    }

    assert!(!uow.has_active_transaction());
}
