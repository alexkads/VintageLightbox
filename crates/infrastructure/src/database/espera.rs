//! 🔒 O catálogo ocupado não vira erro na tela.
//!
//! # Por que isto existe
//!
//! O estúdio de Gramado passou de 25 a 31/out/2026 com *"6 fotos não foram
//! copiadas: … (code: 5) database is locked"* na Nova sessão, e com a revelação
//! e até a leitura de foto recusadas do mesmo jeito. O banco abria no diário
//! clássico do SQLite, em que quem lê segura o arquivo, e a releitura do
//! catálogo inteiro — longa num balcão com semanas de uso — fazia a gravação
//! desistir nos 5 s do sqlx.
//!
//! São três camadas, da mais barata para a mais cara:
//!
//! 1. **WAL** ([`super::create_pool`]): leitura não bloqueia escrita.
//! 2. **A espera do próprio SQLite** (`busy_timeout`, [`ESPERA_DO_SQLITE`]): a
//!    conexão tenta de novo enquanto outra grava.
//! 3. **Esta**: o que ainda voltar "ocupado" — a espera estourada, ou o
//!    `SQLITE_BUSY_SNAPSHOT` do WAL, que o SQLite devolve sem esperar — é
//!    tentado de novo aqui, com recuo crescente, até [`PRAZO`].
//!
//! 🔌 **O pool sem conexão livre também é "ocupado".** A primeira carga que
//! passou limpa do cadeado achou o vizinho dele: com oito releituras do
//! catálogo inteiro ao mesmo tempo, as conexões ficavam todas lendo e a
//! gravação desistia com *"pool timed out while waiting for an open
//! connection"* — para o balconista, a mesma foto que não entra. O pool cresceu
//! ([`CONEXOES`]) e quem ainda assim não achar conexão espera aqui.
//!
//! Só "ocupado" volta; qualquer outro erro sai na hora, como sempre saiu.

use std::future::Future;
use std::time::{Duration, Instant};

/// Quanto cada conexão espera o arquivo sozinha, dentro do SQLite.
pub const ESPERA_DO_SQLITE: Duration = Duration::from_secs(30);

/// Até quando esta camada insiste, somando as tentativas.
pub const PRAZO: Duration = Duration::from_secs(120);

/// As conexões do pool: em WAL os leitores não se atrapalham, e uma releitura
/// longa não pode deixar a gravação sem vez.
pub const CONEXOES: u32 = 16;

/// Quanto um pedido espera por uma conexão livre do pool.
pub const ESPERA_POR_CONEXAO: Duration = Duration::from_secs(60);

/// O primeiro recuo; dobra a cada volta até [`RECUO_MAXIMO`].
const PRIMEIRO_RECUO: Duration = Duration::from_millis(50);
const RECUO_MAXIMO: Duration = Duration::from_secs(2);

/// O banco recusou por estar ocupado, e não por erro do pedido — ou o pool
/// não tinha conexão livre.
///
/// Os códigos primários do SQLite são `SQLITE_BUSY` (5) e `SQLITE_LOCKED` (6);
/// os estendidos guardam o primário no byte de baixo (`BUSY_SNAPSHOT` = 517,
/// `BUSY_RECOVERY` = 261, `LOCKED_SHAREDCACHE` = 262…).
pub fn ocupado(erro: &sqlx::Error) -> bool {
    if matches!(erro, sqlx::Error::PoolTimedOut) {
        return true;
    }
    let Some(banco) = erro.as_database_error() else {
        return false;
    };
    match banco.code().and_then(|c| c.parse::<i32>().ok()) {
        Some(codigo) => matches!(codigo & 0xff, 5 | 6),
        None => {
            let texto = banco.message();
            texto.contains("database is locked") || texto.contains("database table is locked")
        }
    }
}

/// Roda `pedido` e, se o banco estiver ocupado, espera e roda de novo.
///
/// `pedido` monta a consulta **a cada volta** — os `bind` do sqlx consomem a
/// consulta, e uma consulta executada não se executa outra vez. O `o_que`
/// nomeia o gesto no aviso de quando foi preciso insistir.
pub async fn com_espera<T, F, Fut>(o_que: &str, mut pedido: F) -> Result<T, sqlx::Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, sqlx::Error>>,
{
    let inicio = Instant::now();
    let mut recuo = PRIMEIRO_RECUO;
    let mut voltas = 0u32;
    loop {
        match pedido().await {
            Ok(valor) => {
                if voltas > 0 {
                    eprintln!(
                        "🔒 catálogo ocupado: {o_que} entrou na {}ª tentativa, depois de {:.1}s",
                        voltas + 1,
                        inicio.elapsed().as_secs_f32()
                    );
                }
                return Ok(valor);
            }
            Err(erro) if ocupado(&erro) && inicio.elapsed() + recuo < PRAZO => {
                voltas += 1;
                tokio::time::sleep(recuo).await;
                recuo = (recuo * 2).min(RECUO_MAXIMO);
            }
            Err(erro) => return Err(erro),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn so_o_banco_ocupado_ou_o_pool_cheio_contam() {
        assert!(!ocupado(&sqlx::Error::RowNotFound));
        assert!(ocupado(&sqlx::Error::PoolTimedOut));
    }

    /// Um arquivo segurado por outra conexão por 1,5 s — mais que a espera do
    /// SQLite desta, de propósito curta — e a gravação entra mesmo assim.
    #[tokio::test]
    async fn insiste_ate_o_arquivo_soltar() {
        use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
        use std::str::FromStr;
        let pasta = tempfile::tempdir().unwrap();
        let url = format!("sqlite:{}?mode=rwc", pasta.path().join("c.db").display());
        let opcoes = SqliteConnectOptions::from_str(&url)
            .unwrap()
            .busy_timeout(Duration::from_millis(100));
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect_with(opcoes)
            .await
            .unwrap();
        sqlx::query("CREATE TABLE t (x INTEGER)")
            .execute(&pool)
            .await
            .unwrap();

        let mut dono = pool.acquire().await.unwrap();
        sqlx::query("BEGIN EXCLUSIVE")
            .execute(&mut *dono)
            .await
            .unwrap();
        let solta = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            sqlx::query("COMMIT").execute(&mut *dono).await.unwrap();
        });

        let voltas = AtomicU32::new(0);
        com_espera("gravar no teste", || {
            voltas.fetch_add(1, Ordering::Relaxed);
            sqlx::query("INSERT INTO t VALUES (1)").execute(&pool)
        })
        .await
        .expect("entra quando o arquivo solta");
        solta.await.unwrap();
        assert!(voltas.load(Ordering::Relaxed) > 1, "não precisou insistir");
    }

    /// Erro que não é de ocupado sai na primeira, sem esperar.
    #[tokio::test]
    async fn erro_de_verdade_nao_espera() {
        let pool = crate::database::create_pool("sqlite::memory:")
            .await
            .unwrap();
        let voltas = AtomicU32::new(0);
        let inicio = Instant::now();
        let r = com_espera("tabela que não existe", || {
            voltas.fetch_add(1, Ordering::Relaxed);
            sqlx::query("INSERT INTO nao_existe VALUES (1)").execute(&pool)
        })
        .await;
        assert!(r.is_err());
        assert_eq!(voltas.load(Ordering::Relaxed), 1);
        assert!(inicio.elapsed() < Duration::from_millis(200));
    }
}
