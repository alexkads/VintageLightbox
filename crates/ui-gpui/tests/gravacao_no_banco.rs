//! A gravação da Revelação, com banco de verdade.
//!
//! # Por que este teste existe, se os da tela já cobrem a gravação
//!
//! Os testes de `revelacao::tela` usam um gravador de mentira: eles provam
//! **quando** a tela manda gravar, e com quais valores. Nada neles toca no
//! `EditorController`, no use case, na entidade ou no SQLite — e é justamente
//! nessa metade que mora o defeito que mais custa caro:
//!
//! 🚨 **`SavePhotoEditsUseCase` apaga o corte quando recebe `None`.** A entidade
//! faz `self.edit_crop_x = crop_x`, atribuição direta. Como a Revelação em GPUI
//! ainda não tem crop overlay, ela não teria motivo nenhum para mandar corte — e
//! mexer num slider apagaria, calado, o enquadramento feito no app de egui. Aqui
//! isso é conferido no banco, lendo a linha depois.
//!
//! O outro motivo é o `tokio::runtime::Handle`: o `GravadorDoBanco` despacha a
//! gravação para o runtime, e um `Handle` errado não falha na compilação — falha
//! em tempo de execução, dentro de uma tarefa, sem ninguém olhando.

use std::sync::Arc;
use std::time::Duration;

use adapters::controllers::{EditorController, LibraryController};
use ui_gpui::revelacao::persistencia::{self, Corte, Gravador, GravadorDoBanco};
use ui_gpui::revelacao::processador::Ajustes;
use use_cases::SavePhotoEditsUseCase;

/// Um banco temporário com as migrations aplicadas e uma foto dentro.
async fn banco_com_uma_foto(dir: &tempfile::TempDir, id: &str) -> sqlx::SqlitePool {
    let caminho = dir.path().join("catalogo.db");
    let pool = infrastructure::create_pool(&format!("sqlite:{}?mode=rwc", caminho.display()))
        .await
        .expect("abrir o banco");
    infrastructure::run_migrations(&pool)
        .await
        .expect("rodar as migrations");

    let agora = chrono::Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO photos (id, file_path, rating, is_edited, imported_at, modified_at)
         VALUES (?1, ?2, 0, 0, ?3, ?3)",
    )
    .bind(id)
    .bind("/fotos/DSC_0001.NEF")
    .bind(&agora)
    .execute(&pool)
    .await
    .expect("inserir a foto");

    pool
}

fn gravador_de(pool: &sqlx::SqlitePool) -> GravadorDoBanco {
    let repositorio = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    let editor = Arc::new(EditorController::new(
        Arc::new(SavePhotoEditsUseCase::new(repositorio)),
        Arc::new(use_cases::pos_venda::RevelacoesLocaisUseCase::new(
            Arc::new(infrastructure::SqliteRevelacoesDoSite::new(pool.clone())),
        )),
    ));
    GravadorDoBanco::novo(editor, tokio::runtime::Handle::current(), Vec::new())
}

/// Relê a foto pelo mesmo caminho que o app usa para abrir a Revelação.
async fn reler(pool: &sqlx::SqlitePool, id: &str) -> adapters::view_models::PhotoViewModel {
    let repositorio = Arc::new(infrastructure::PhotoRepositoryImpl::new(pool.clone()));
    LibraryController::new(repositorio)
        .get_all_photos()
        .await
        .expect("reler as fotos")
        .into_iter()
        .find(|foto| foto.id == id)
        .expect("a foto tem de estar lá")
}

/// A gravação é despachada para o tokio e volta sem `Result` — então o teste
/// espera a linha mudar, em vez de adivinhar um tempo fixo.
async fn esperar_gravacao(pool: &sqlx::SqlitePool, id: &str) {
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let gravado: Option<f32> =
            sqlx::query_scalar("SELECT edit_exposure FROM photos WHERE id = ?1")
                .bind(id)
                .fetch_one(pool)
                .await
                .expect("ler a exposição");
        if gravado.is_some() {
            return;
        }
    }
    panic!("a gravação não chegou ao banco em 1s");
}

/// 🚨 O que a tela gravou volta do banco igual, pelos 53 campos.
///
/// É a ida e volta inteira: `Ajustes` → controller → use case → entidade →
/// SQLite → `row_to_photo` → `PhotoViewModel` → `da_foto`. Sete etapas, e cada
/// uma delas com nomes parecidos o bastante para trocar `hsl_blue_lum` por
/// `hsl_blue_sat` sem ninguém ver.
#[tokio::test]
async fn a_revelacao_gravada_volta_inteira() {
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let id = uuid::Uuid::new_v4().to_string();
    let pool = banco_com_uma_foto(&dir, &id).await;

    // Valores diferentes campo a campo: se dois deles fossem iguais, uma troca
    // entre os dois passaria despercebida.
    let mut ajustes = Ajustes::default();
    let campos: &mut [f32] = bytemuck::cast_slice_mut(bytemuck::bytes_of_mut(&mut ajustes));
    for (i, campo) in campos.iter_mut().enumerate() {
        *campo = 0.5 + i as f32;
    }

    gravador_de(&pool).gravar(id.clone(), ajustes, Corte::default());
    esperar_gravacao(&pool, &id).await;

    let de_volta = persistencia::da_foto(&reler(&pool, &id).await);
    assert_eq!(
        de_volta, ajustes,
        "algum dos 53 campos não sobreviveu à ida e volta"
    );
}

/// 🚨 Gravar ajuste **não apaga o corte** que a foto já tinha.
///
/// O caminho que este teste percorre é o do defeito: uma foto cortada no app de
/// egui, aberta na Revelação nova (que não sabe cortar), e um slider mexido. Se o
/// corte não fosse reenviado, a linha voltaria com os oito campos em `NULL` — e o
/// enquadramento sumiria sem erro, sem aviso e sem desfazer.
#[tokio::test]
async fn gravar_ajuste_nao_apaga_o_corte() {
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let id = uuid::Uuid::new_v4().to_string();
    let pool = banco_com_uma_foto(&dir, &id).await;

    sqlx::query(
        "UPDATE photos SET edit_crop_x = 0.1, edit_crop_y = 0.2, edit_crop_width = 0.5,
                           edit_crop_height = 0.4, edit_crop_rotation = 90,
                           edit_crop_angle = -2.5, edit_crop_flip_h = 1, edit_crop_flip_v = 0
         WHERE id = ?1",
    )
    .bind(&id)
    .execute(&pool)
    .await
    .expect("cortar a foto como o app de egui cortaria");

    // O que a Revelação faz ao abrir: lê o corte da foto para devolvê-lo depois.
    let corte = persistencia::corte_da_foto(&reler(&pool, &id).await);
    assert_eq!(corte.x, Some(0.1), "o corte tem de chegar à tela primeiro");

    let ajustes = Ajustes {
        exposure: 1.25,
        ..Default::default()
    };
    gravador_de(&pool).gravar(id.clone(), ajustes, corte);
    esperar_gravacao(&pool, &id).await;

    let foto = reler(&pool, &id).await;
    assert_eq!(foto.edit_exposure, Some(1.25), "o ajuste foi gravado");
    assert_eq!(
        persistencia::corte_da_foto(&foto),
        corte,
        "o corte sobreviveu"
    );
}

/// 🚨 A contraprova: sem reenviar o corte, ele **é apagado**.
///
/// Sem este teste, o de cima poderia estar passando por engano — se o use case
/// mesclasse em vez de sobrescrever, reenviar o corte não seria necessário e
/// ninguém saberia. Aqui está a prova de que a precaução é a única coisa que
/// separa o enquadramento de sumir.
#[tokio::test]
async fn sem_reenviar_o_corte_ele_e_apagado() {
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let id = uuid::Uuid::new_v4().to_string();
    let pool = banco_com_uma_foto(&dir, &id).await;

    sqlx::query("UPDATE photos SET edit_crop_x = 0.1, edit_crop_width = 0.5 WHERE id = ?1")
        .bind(&id)
        .execute(&pool)
        .await
        .expect("cortar a foto");

    gravador_de(&pool).gravar(
        id.clone(),
        Ajustes {
            exposure: 1.25,
            ..Default::default()
        },
        Corte::default(),
    );
    esperar_gravacao(&pool, &id).await;

    let foto = reler(&pool, &id).await;
    assert_eq!(
        foto.edit_crop_x, None,
        "se isto passar a sobreviver, o use case mudou — e o reenvio do corte deixou de ser necessário"
    );
}

/// 🚨 **A foto do site grava no depósito, e não em `photos`.**
///
/// Achado do dono em 8/set/2026: *"parece que os parâmetros de edição não estão
/// sendo gravados"*. O id de uma foto aberta de uma sessão do pós-venda é
/// `site:<uuid>`, que não é linha do catálogo — `SavePhotoEditsUseCase`
/// respondia `PhotoNotFound` e o `Gravador` engolia (não devolve `Result`, de
/// propósito: gravar acontece 500 ms depois do arrasto, longe de quem
/// arrastou). Cada gesto escrevia na água.
///
/// O teste confere as duas metades: o que **entra** no depósito e o que **não
/// entra** no catálogo — uma linha em `photos` para uma foto que não está neste
/// disco seria o outro erro possível.
#[tokio::test]
async fn a_revelacao_da_foto_do_site_vai_para_o_deposito() {
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let id = uuid::Uuid::new_v4().to_string();
    let pool = banco_com_uma_foto(&dir, &id).await;

    gravador_de(&pool).gravar(
        "site:remota-1".to_string(),
        Ajustes {
            exposure: 1.25,
            ..Default::default()
        },
        Corte::default(),
    );

    let guardado = esperar_o_deposito(&pool, "remota-1").await;
    let ajustes: serde_json::Value = serde_json::from_str(&guardado).expect("é JSON");
    assert_eq!(ajustes["exposure"], 1.25);
    // 🔑 O enquadramento vai no mesmo objeto, com prefixo `corte_` — o formato
    // que sobe para a API. Um segundo formato aqui faria a foto revelada no app
    // abrir sem enquadramento no navegador.
    assert_eq!(ajustes["corte_largura"], 1.0);

    let fotos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM photos")
        .fetch_one(&pool)
        .await
        .expect("contar as fotos");
    assert_eq!(fotos, 1, "a foto do site não vira linha do catálogo local");
}

/// Espera o depósito responder — a gravação é assíncrona, como a do catálogo.
async fn esperar_o_deposito(pool: &sqlx::SqlitePool, foto_no_site: &str) -> String {
    for _ in 0..50 {
        let guardado: Option<String> = sqlx::query_scalar(
            "SELECT ajustes FROM revelacoes_do_site WHERE pos_venda_foto_id = ?1",
        )
        .bind(foto_no_site)
        .fetch_optional(pool)
        .await
        .expect("ler o depósito");
        if let Some(ajustes) = guardado {
            return ajustes;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("o depósito não recebeu a revelação de {foto_no_site}");
}
