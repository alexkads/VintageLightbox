//! 🧹 **As fotos locais que já estão no site** — o que a limpeza por período
//! (Configurações › Cache) apaga além do cache (issue #22, 17/11/2026).
//!
//! > *"Eu preciso que limpar, também limpe as fotos e não somente o cache"* ·
//! > *"precisa limpar somente as fotos locais, pois o disco do PC está
//! > lotado"* — dono, 17/11/2026.
//!
//! # Por que existe
//!
//! A sessão importa **copiando** para `<catálogo>/Ensaios/<título> - <id>/`, e
//! a cópia continua lá depois de subir (C20.1). Num balcão com meses de
//! sessões, é ela — e não o cache de 200 MB — que enche o disco. Sem a cópia, a
//! foto abre como **foto do site** (o bruto vem do storage), o mesmo caminho da
//! que subiu de outra máquina.
//!
//! # A regra: só sai o que o site já guarda, e nada que ainda dependa daqui
//!
//! Uma foto local sai quando, ao mesmo tempo:
//!
//! - é de uma sessão (`sessao_id`) e **já subiu** (`pos_venda_foto_id`) — a
//!   foto da Biblioteca, que nunca foi ao site, nunca sai;
//! - **não foi mexida no período** (`modified_at`): nota, revelação ou corte
//!   renovam a data, e a sessão de hoje fica;
//! - **não está rejeitada** (`flag = -1`): a rejeitada saiu da nuvem
//!   (`app/resgate.rs`), e a cópia daqui é a única que existe;
//! - **não tem gesto na fila de sincronização**, pendente ou em conflito — pela
//!   sessão, pelo id local ou pelo id no site: o que ainda vai subir lê daqui;
//! - **não tem projeto de edição em camadas** (`edicoes_de_foto`): o projeto
//!   mora ao lado do catálogo e parte do arquivo.
//!
//! 🔑 **A linha sai junto com o arquivo.** Apagar só o arquivo deixaria no
//! catálogo uma foto local apontando para o nada, e a Revelação tentaria
//! repor o preview de um arquivo que não existe. Sem a linha, ela é a foto do
//! site, que já sabe de onde vêm os pixels.
//!
//! O **espelho do site** (as leituras guardadas para trabalhar sem rede, com as
//! miniaturas e as cópias de trabalho) é aparado pelo mesmo período: ele se
//! refaz na próxima leitura com rede.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::database::espera;

/// O `flag` da foto rejeitada (`biblioteca::marcacao::REJEITADA_NO_CATALOGO`).
const REJEITADA: i64 = -1;

/// O que a limpeza levaria (ou levou): as fotos e o espelho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Retrato {
    /// Fotos locais que saem.
    pub fotos: u64,
    /// Quanto elas ocupam no disco.
    pub bytes_das_fotos: u64,
    /// Arquivos do espelho do site sem leitura no período.
    pub do_espelho: u64,
    pub bytes_do_espelho: u64,
}

impl Retrato {
    pub fn bytes(&self) -> u64 {
        self.bytes_das_fotos + self.bytes_do_espelho
    }

    pub fn vazio(&self) -> bool {
        self.fotos == 0 && self.do_espelho == 0
    }
}

/// Uma foto local que pode sair.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidata {
    id: String,
    caminho: PathBuf,
}

/// As fotos locais do catálogo e o espelho do site.
pub struct FotosLocais {
    pool: SqlitePool,
    /// `<catálogo>/Espelho do site` — todas as pastas de endereço juntas.
    espelho: Option<PathBuf>,
}

impl FotosLocais {
    pub fn new(pool: SqlitePool, espelho: Option<PathBuf>) -> Self {
        Self { pool, espelho }
    }

    /// O que sairia, sem apagar nada — o número que a tela mostra antes do clique.
    pub async fn sem_uso_ha(&self, dias: u32) -> Result<Retrato, String> {
        let agora = Utc::now();
        let candidatas = self.candidatas(limite(dias, agora)).await?;
        let (do_espelho, bytes_do_espelho) = self
            .espelho
            .as_deref()
            .map(|pasta| varrer_espelho(pasta, dias, false))
            .unwrap_or((0, 0));
        Ok(Retrato {
            fotos: candidatas.len() as u64,
            bytes_das_fotos: candidatas.iter().map(|c| tamanho(&c.caminho)).sum(),
            do_espelho,
            bytes_do_espelho,
        })
    }

    /// Apaga o que está sem uso há mais de `dias` e devolve o que saiu.
    pub async fn limpar_sem_uso_ha(&self, dias: u32) -> Result<Retrato, String> {
        let candidatas = self.candidatas(limite(dias, Utc::now())).await?;
        let mut retrato = Retrato::default();
        let mut pastas = HashSet::new();
        for candidata in candidatas {
            let bytes = tamanho(&candidata.caminho);
            // 🔑 **A linha primeiro.** Se o arquivo não sair (aberto em outro
            // programa no Windows), sobra um arquivo solto na pasta do ensaio —
            // espaço, e não uma foto quebrada no catálogo.
            let id = candidata.id.clone();
            let apagada = espera::com_espera("limpar fotos locais", || {
                sqlx::query("DELETE FROM photos WHERE id = ?")
                    .bind(&id)
                    .execute(&self.pool)
            })
            .await
            .map_err(|e| e.to_string())?;
            if apagada.rows_affected() == 0 {
                continue;
            }
            retrato.fotos += 1;
            match std::fs::remove_file(&candidata.caminho) {
                Ok(()) => retrato.bytes_das_fotos += bytes,
                Err(erro) if erro.kind() == std::io::ErrorKind::NotFound => {}
                Err(erro) => eprintln!(
                    "⚠️ [Limpeza] {} saiu do catálogo, mas o arquivo ficou: {erro}",
                    candidata.caminho.display()
                ),
            }
            if let Some(pasta) = candidata.caminho.parent() {
                pastas.insert(pasta.to_path_buf());
            }
        }
        // A pasta do ensaio que ficou vazia sai também — `remove_dir` recusa
        // pasta com qualquer coisa dentro, e é essa recusa que a protege.
        for pasta in pastas {
            let _ = std::fs::remove_dir(&pasta);
        }
        if let Some(pasta) = self.espelho.as_deref() {
            let (quantos, bytes) = varrer_espelho(pasta, dias, true);
            retrato.do_espelho = quantos;
            retrato.bytes_do_espelho = bytes;
        }
        Ok(retrato)
    }

    async fn candidatas(&self, limite: DateTime<Utc>) -> Result<Vec<Candidata>, String> {
        let linhas = espera::com_espera("fotos locais sem uso", || {
            sqlx::query(
                "SELECT p.id, p.file_path, p.modified_at FROM photos p
                 WHERE p.sessao_id IS NOT NULL AND p.sessao_id <> ''
                   AND p.pos_venda_foto_id IS NOT NULL AND p.pos_venda_foto_id <> ''
                   AND COALESCE(p.flag, 0) <> ?
                   AND NOT EXISTS (
                       SELECT 1 FROM fila_de_sincronizacao f
                       WHERE f.galeria_id = p.sessao_id
                          OR f.alvo = p.id
                          OR f.alvo = p.pos_venda_foto_id)
                   AND NOT EXISTS (
                       SELECT 1 FROM edicoes_de_foto e
                       WHERE e.foto_id = p.id
                          OR e.pos_venda_foto_id = p.pos_venda_foto_id)",
            )
            .bind(REJEITADA)
            .fetch_all(&self.pool)
        })
        .await
        .map_err(|e| e.to_string())?;
        Ok(linhas
            .into_iter()
            .filter(|linha| {
                // ⚠️ A data é lida, e não comparada como texto: o RFC 3339 com
                // fuso diferente de UTC não ordena como texto.
                linha
                    .try_get::<String, _>("modified_at")
                    .ok()
                    .and_then(|texto| DateTime::parse_from_rfc3339(&texto).ok())
                    .is_some_and(|mexida| mexida.with_timezone(&Utc) < limite)
            })
            .filter_map(|linha| {
                let caminho: String = linha.try_get("file_path").ok()?;
                // Foto do site sem arquivo aqui tem `path` vazio: não há o que
                // liberar, e a linha é a que guarda a revelação dela.
                (!caminho.is_empty()).then(|| Candidata {
                    id: linha.get("id"),
                    caminho: PathBuf::from(caminho),
                })
            })
            .collect())
    }
}

fn limite(dias: u32, agora: DateTime<Utc>) -> DateTime<Utc> {
    agora - chrono::Duration::days(i64::from(dias))
}

fn tamanho(caminho: &Path) -> u64 {
    std::fs::metadata(caminho).map(|m| m.len()).unwrap_or(0)
}

/// Conta (e, com `apagar`, apaga) os arquivos do espelho não regravados há mais
/// de `dias` — a mesma régua da faxina da abertura (`EspelhoEmDisco::faxina`),
/// em todas as pastas de endereço.
fn varrer_espelho(pasta: &Path, dias: u32, apagar: bool) -> (u64, u64) {
    let limite = SystemTime::now() - Duration::from_secs(u64::from(dias.max(1)) * 86_400);
    let Ok(enderecos) = std::fs::read_dir(pasta) else {
        return (0, 0);
    };
    let mut quantos = 0;
    let mut bytes = 0;
    for endereco in enderecos.flatten() {
        let Ok(entradas) = std::fs::read_dir(endereco.path()) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let Ok(meta) = entrada.metadata() else {
                continue;
            };
            let velho = meta.is_file() && meta.modified().map(|m| m < limite).unwrap_or(false);
            if !velho || (apagar && std::fs::remove_file(entrada.path()).is_err()) {
                continue;
            }
            quantos += 1;
            bytes += meta.len();
        }
    }
    (quantos, bytes)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::database::{create_pool, run_migrations};
    use tempfile::TempDir;

    struct Catalogo {
        pool: SqlitePool,
        pasta: TempDir,
    }

    async fn catalogo() -> Catalogo {
        let pasta = TempDir::new().unwrap();
        let url = format!("sqlite://{}?mode=rwc", pasta.path().join("c.db").display());
        let pool = create_pool(&url).await.unwrap();
        run_migrations(&pool).await.unwrap();
        Catalogo { pool, pasta }
    }

    impl Catalogo {
        /// Uma foto de sessão com arquivo de verdade em `Ensaios/<sessão>/`.
        async fn foto(
            &self,
            id: &str,
            sessao: Option<&str>,
            no_site: Option<&str>,
            dias_parada: i64,
        ) -> PathBuf {
            let pasta = self
                .pasta
                .path()
                .join("Ensaios")
                .join(sessao.unwrap_or("biblioteca"));
            std::fs::create_dir_all(&pasta).unwrap();
            let arquivo = pasta.join(format!("{id}.jpg"));
            std::fs::write(&arquivo, vec![7u8; 1000]).unwrap();
            let quando = (Utc::now() - chrono::Duration::days(dias_parada)).to_rfc3339();
            sqlx::query(
                "INSERT INTO photos (id, file_path, is_edited, imported_at, modified_at, \
                 sessao_id, pos_venda_foto_id) VALUES (?, ?, 0, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(arquivo.to_string_lossy().to_string())
            .bind(&quando)
            .bind(&quando)
            .bind(sessao)
            .bind(no_site)
            .execute(&self.pool)
            .await
            .unwrap();
            arquivo
        }

        fn fotos(&self) -> FotosLocais {
            FotosLocais::new(self.pool.clone(), None)
        }

        async fn existe(&self, id: &str) -> bool {
            sqlx::query("SELECT 1 FROM photos WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await
                .unwrap()
                .is_some()
        }
    }

    /// 🚨 A foto que já está no site e está parada sai — arquivo, linha e a
    /// pasta do ensaio que ficou vazia.
    #[tokio::test]
    async fn a_foto_ja_no_site_e_parada_sai_com_a_pasta() {
        let c = catalogo().await;
        let arquivo = c.foto("a", Some("s1"), Some("site-a"), 40).await;

        let antes = c.fotos().sem_uso_ha(30).await.unwrap();
        assert_eq!((antes.fotos, antes.bytes_das_fotos), (1, 1000));

        let saiu = c.fotos().limpar_sem_uso_ha(30).await.unwrap();
        assert_eq!((saiu.fotos, saiu.bytes_das_fotos), (1, 1000));
        assert!(!arquivo.exists(), "o arquivo sai");
        assert!(!c.existe("a").await, "a linha sai junto");
        assert!(!arquivo.parent().unwrap().exists(), "a pasta vazia sai");
    }

    /// 🚨 O período manda: a foto mexida há 10 dias fica nos 15, 30, 60 e 90.
    #[tokio::test]
    async fn a_foto_mexida_no_periodo_fica() {
        let c = catalogo().await;
        let arquivo = c.foto("a", Some("s1"), Some("site-a"), 10).await;

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 0);
        assert!(arquivo.exists());
        assert!(c.existe("a").await);
    }

    /// 🚨 O que o site não guarda nunca sai: a foto que não subiu e a da
    /// Biblioteca, sem sessão.
    #[tokio::test]
    async fn o_que_nao_esta_no_site_fica() {
        let c = catalogo().await;
        let sem_subir = c.foto("a", Some("s1"), None, 90).await;
        let biblioteca = c.foto("b", None, None, 90).await;

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 0);
        assert!(sem_subir.exists() && biblioteca.exists());
    }

    /// 🚨 A rejeitada saiu da nuvem: a cópia daqui é a única.
    #[tokio::test]
    async fn a_rejeitada_fica() {
        let c = catalogo().await;
        let arquivo = c.foto("a", Some("s1"), Some("site-a"), 90).await;
        sqlx::query("UPDATE photos SET flag = -1 WHERE id = 'a'")
            .execute(&c.pool)
            .await
            .unwrap();

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 0);
        assert!(arquivo.exists());
    }

    /// 🚨 Gesto na fila (pela sessão, pelo id local ou pelo id no site) segura
    /// a foto: o que ainda vai subir lê daqui.
    #[tokio::test]
    async fn gesto_na_fila_segura_a_foto() {
        let c = catalogo().await;
        let pela_sessao = c.foto("a", Some("s1"), Some("site-a"), 90).await;
        let pelo_local = c.foto("b", Some("s2"), Some("site-b"), 90).await;
        let pelo_site = c.foto("c", Some("s3"), Some("site-c"), 90).await;
        let livre = c.foto("d", Some("s4"), Some("site-d"), 90).await;
        for (chave, alvo, galeria, estado) in [
            ("1", "x", Some("s1"), "pendente"),
            ("2", "b", None, "conflito"),
            ("3", "site-c", None, "pendente"),
        ] {
            sqlx::query(
                "INSERT INTO fila_de_sincronizacao (chave, tipo, alvo, galeria_id, corpo, \
                 ocorrido_em, estado) VALUES (?, 'MudarFoto', ?, ?, '{}', '2026-11-17', ?)",
            )
            .bind(chave)
            .bind(alvo)
            .bind(galeria)
            .bind(estado)
            .execute(&c.pool)
            .await
            .unwrap();
        }

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 1);
        assert!(pela_sessao.exists() && pelo_local.exists() && pelo_site.exists());
        assert!(!livre.exists());
    }

    /// 🚨 O projeto de edição em camadas parte do arquivo: a foto fica.
    #[tokio::test]
    async fn a_foto_com_edicao_em_camadas_fica() {
        let c = catalogo().await;
        let arquivo = c.foto("a", Some("s1"), Some("site-a"), 90).await;
        sqlx::query(
            "INSERT INTO edicoes_de_foto (edicao_id, foto_id, diretorio, base_sha256) \
             VALUES ('e1', 'a', '/x', 'h')",
        )
        .execute(&c.pool)
        .await
        .unwrap();

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 0);
        assert!(arquivo.exists());
    }

    /// A pasta do ensaio com outra coisa dentro não sai.
    #[tokio::test]
    async fn a_pasta_com_outra_foto_fica() {
        let c = catalogo().await;
        let velha = c.foto("a", Some("s1"), Some("site-a"), 90).await;
        let nova = c.foto("b", Some("s1"), Some("site-b"), 1).await;

        assert_eq!(c.fotos().limpar_sem_uso_ha(30).await.unwrap().fotos, 1);
        assert!(!velha.exists());
        assert!(nova.exists(), "a recente fica, e a pasta com ela");
    }

    /// O espelho velho sai pelo mesmo período; o recém-lido fica.
    #[tokio::test]
    async fn o_espelho_velho_sai_e_o_recente_fica() {
        let c = catalogo().await;
        let espelho = c.pasta.path().join("Espelho do site");
        let endereco = espelho.join("api");
        std::fs::create_dir_all(&endereco).unwrap();
        let velho = endereco.join("velho.corpo");
        let novo = endereco.join("novo.corpo");
        std::fs::write(&velho, vec![1u8; 500]).unwrap();
        std::fs::write(&novo, vec![1u8; 300]).unwrap();
        let ha_40_dias = SystemTime::now() - Duration::from_secs(40 * 86_400);
        std::fs::File::options()
            .write(true)
            .open(&velho)
            .unwrap()
            .set_modified(ha_40_dias)
            .unwrap();

        let fotos = FotosLocais::new(c.pool.clone(), Some(espelho));
        let antes = fotos.sem_uso_ha(30).await.unwrap();
        assert_eq!((antes.do_espelho, antes.bytes_do_espelho), (1, 500));
        assert!(velho.exists(), "medir não apaga");

        let saiu = fotos.limpar_sem_uso_ha(30).await.unwrap();
        assert_eq!((saiu.do_espelho, saiu.bytes()), (1, 500));
        assert!(!velho.exists() && novo.exists());
    }
}
