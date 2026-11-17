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
//! # A regra: o período, e nada mais (dono, 17/11/2026, #28)
//!
//! > *"Independe de eles estarem no site ou não, se disser que tenho que
//! > excluir as 30 dias para trás, eles devem ser excluídas"* · *"Excluir
//! > somente as fotos locais da máquina e não do site! [...] não preciso que
//! > você faça essa verificação."*
//!
//! Sai **a cópia local** de toda foto parada há mais que o período
//! (`modified_at`: nota, revelação ou corte renovam a data). No site nada muda,
//! e o app **não confere** se a foto está lá: a que não subiu, a rejeitada, a
//! da Biblioteca e a que tem edição saem do mesmo jeito. A primeira versão
//! (#22) segurava todas elas, e o balcão seguia cheio.
//!
//! Duas travas, nenhuma ligada ao site:
//!
//! - **Só arquivo de dentro da pasta do catálogo.** A importação copia para lá;
//!   a foto trazida do catálogo do Lightroom aponta para o original do
//!   fotógrafo, que não é do app — essa fica, com a linha.
//! - **Gesto na fila de sincronização segura a foto** — pela sessão, pelo id
//!   local ou pelo id no site. O envio em andamento lê daqui; a foto sai no
//!   clique seguinte, depois de a fila andar.
//!
//! 🔑 **A linha sai junto com o arquivo.** Apagar só o arquivo deixaria no
//! catálogo uma foto local apontando para o nada, e a Revelação tentaria
//! repor o preview de um arquivo que não existe. Sem a linha, a foto que está
//! no site volta como foto do site, que já sabe de onde vêm os pixels.
//!
//! # O espelho do site
//!
//! As leituras guardadas para trabalhar sem rede (`pos_venda::espelho`) são
//! aparadas pelo mesmo período. A resposta `image/*` — a miniatura e a cópia de
//! trabalho de cada foto das sessões abertas — **conta como foto**: é onde
//! estão quase todas as fotos de um balcão que não importou a sessão (439 MB
//! no Fedora, 773 MB no Mac, em 17/11). O resto (`json`) conta como item. Tudo
//! se refaz na próxima leitura com rede.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};

use crate::database::espera;

/// O que a limpeza levaria (ou levou): as fotos e o espelho.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Retrato {
    /// Fotos locais que saem.
    pub fotos: u64,
    /// Quanto elas ocupam no disco.
    pub bytes_das_fotos: u64,
    /// Fotos do site guardadas no espelho (respostas `image/*`), sem leitura
    /// no período — cada uma é o corpo e o cabeçalho dela.
    pub fotos_do_espelho: u64,
    pub bytes_das_fotos_do_espelho: u64,
    /// Os outros arquivos do espelho (os dados) sem leitura no período.
    pub do_espelho: u64,
    pub bytes_do_espelho: u64,
}

impl Retrato {
    pub fn bytes(&self) -> u64 {
        self.bytes_das_fotos + self.bytes_das_fotos_do_espelho + self.bytes_do_espelho
    }

    /// As fotos do catálogo e as do espelho: o que a tela chama de "fotos".
    pub fn todas_as_fotos(&self) -> u64 {
        self.fotos + self.fotos_do_espelho
    }

    pub fn vazio(&self) -> bool {
        self.todas_as_fotos() == 0 && self.do_espelho == 0
    }
}

/// Quanto da limpeza já andou — o que a barra do rodapé mostra (#25).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Andamento {
    /// Fotos já tiradas do catálogo.
    pub feitas: u64,
    /// Quantas fotos a limpeza vai tirar.
    pub total: u64,
    /// Bytes de foto já liberados.
    pub bytes: u64,
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
    /// A pasta do catálogo: só arquivo daqui de dentro sai.
    catalogo: PathBuf,
    /// `<catálogo>/Espelho do site` — todas as pastas de endereço juntas.
    espelho: PathBuf,
}

impl FotosLocais {
    pub fn new(pool: SqlitePool, catalogo: PathBuf) -> Self {
        let espelho = catalogo.join("Espelho do site");
        Self {
            pool,
            catalogo,
            espelho,
        }
    }

    /// 📸 **Todas as fotos guardadas nesta máquina**, sem período — o total
    /// que a aba Armazenamento mostra (#28): as do catálogo com arquivo aqui
    /// dentro e as do site no espelho. Os dados do espelho não entram.
    pub async fn guardadas(&self) -> Result<Retrato, String> {
        let linhas = espera::com_espera("fotos guardadas", || {
            sqlx::query("SELECT file_path FROM photos WHERE file_path <> ''").fetch_all(&self.pool)
        })
        .await
        .map_err(|e| e.to_string())?;
        let mut retrato = varrer_espelho(&self.espelho, None, false);
        retrato.do_espelho = 0;
        retrato.bytes_do_espelho = 0;
        for linha in linhas {
            let caminho = PathBuf::from(linha.get::<String, _>("file_path"));
            if !caminho.starts_with(&self.catalogo) {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&caminho) {
                retrato.fotos += 1;
                retrato.bytes_das_fotos += meta.len();
            }
        }
        Ok(retrato)
    }

    /// O que sairia, sem apagar nada — o número que a tela mostra antes do clique.
    pub async fn sem_uso_ha(&self, dias: u32) -> Result<Retrato, String> {
        let agora = Utc::now();
        let candidatas = self.candidatas(limite(dias, agora)).await?;
        let mut retrato = varrer_espelho(&self.espelho, Some(dias), false);
        retrato.fotos = candidatas.len() as u64;
        retrato.bytes_das_fotos = candidatas.iter().map(|c| tamanho(&c.caminho)).sum();
        Ok(retrato)
    }

    /// Apaga o que está sem uso há mais de `dias` e devolve o que saiu.
    pub async fn limpar_sem_uso_ha(&self, dias: u32) -> Result<Retrato, String> {
        self.limpar_sem_uso_ha_contando(dias, |_| {}).await
    }

    /// O mesmo, contando o andamento a cada foto (#25): `andou` recebe o
    /// total logo no começo e depois cada foto que sai.
    ///
    /// 🔑 **Cede a vez ao runtime entre uma foto e outra.** São centenas de
    /// `DELETE` e de arquivos apagados seguidos: sem a pausa, a limpeza
    /// ocuparia uma thread do tokio e o catálogo de ponta a ponta, e o balcão
    /// sentiria na triagem e na revelação, que gravam no mesmo banco.
    pub async fn limpar_sem_uso_ha_contando(
        &self,
        dias: u32,
        andou: impl Fn(Andamento),
    ) -> Result<Retrato, String> {
        let candidatas = self.candidatas(limite(dias, Utc::now())).await?;
        let mut retrato = Retrato::default();
        let mut pastas = HashSet::new();
        let mut andamento = Andamento {
            total: candidatas.len() as u64,
            ..Andamento::default()
        };
        andou(andamento);
        for candidata in candidatas {
            tokio::task::yield_now().await;
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
            andamento.feitas = retrato.fotos;
            andamento.bytes = retrato.bytes_das_fotos;
            andou(andamento);
        }
        // A pasta do ensaio que ficou vazia sai também — `remove_dir` recusa
        // pasta com qualquer coisa dentro, e é essa recusa que a protege.
        for pasta in pastas {
            let _ = std::fs::remove_dir(&pasta);
        }
        let espelho = varrer_espelho(&self.espelho, Some(dias), true);
        retrato.fotos_do_espelho = espelho.fotos_do_espelho;
        retrato.bytes_das_fotos_do_espelho = espelho.bytes_das_fotos_do_espelho;
        retrato.do_espelho = espelho.do_espelho;
        retrato.bytes_do_espelho = espelho.bytes_do_espelho;
        Ok(retrato)
    }

    async fn candidatas(&self, limite: DateTime<Utc>) -> Result<Vec<Candidata>, String> {
        let linhas = espera::com_espera("fotos locais sem uso", || {
            sqlx::query(
                "SELECT p.id, p.file_path, p.modified_at FROM photos p
                 WHERE NOT EXISTS (
                       SELECT 1 FROM fila_de_sincronizacao f
                       WHERE f.galeria_id = p.sessao_id
                          OR f.alvo = p.id
                          OR f.alvo = p.pos_venda_foto_id)",
            )
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
                let caminho = PathBuf::from(caminho);
                // 🚨 Só a cópia que o app fez: o original do fotógrafo, fora
                // do catálogo, nunca sai (nem a linha que aponta para ele).
                (caminho.starts_with(&self.catalogo)).then(|| Candidata {
                    id: linha.get("id"),
                    caminho,
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
/// em todas as pastas de endereço. A resposta `image/*` conta como foto (o
/// corpo e o cabeçalho juntos); o resto, arquivo a arquivo, como item. Sem
/// `dias`, conta tudo (o total da tela; nunca com `apagar`).
fn varrer_espelho(pasta: &Path, dias: Option<u32>, apagar: bool) -> Retrato {
    debug_assert!(dias.is_some() || !apagar, "apagar sempre tem período");
    let limite =
        dias.map(|dias| SystemTime::now() - Duration::from_secs(u64::from(dias.max(1)) * 86_400));
    let mut retrato = Retrato::default();
    let Ok(enderecos) = std::fs::read_dir(pasta) else {
        return retrato;
    };
    let velho = |meta: &std::fs::Metadata| match limite {
        None => true,
        Some(limite) => meta.modified().map(|m| m < limite).unwrap_or(false),
    };
    for endereco in enderecos.flatten() {
        let Ok(entradas) = std::fs::read_dir(endereco.path()) else {
            continue;
        };
        for entrada in entradas.flatten() {
            let caminho = entrada.path();
            let Ok(meta) = entrada.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let extensao = caminho.extension().and_then(|e| e.to_str());
            // O cabeçalho de uma foto anda com o corpo dela, logo abaixo; o
            // que ficou sem corpo é arquivo solto e segue a régua dos itens.
            if extensao == Some("cab")
                && caminho.with_extension("corpo").exists()
                && e_foto(&caminho)
            {
                continue;
            }
            if extensao == Some("corpo") && e_foto(&caminho.with_extension("cab")) {
                if !velho(&meta) {
                    continue;
                }
                let cab = caminho.with_extension("cab");
                let bytes = meta.len() + tamanho(&cab);
                if apagar && std::fs::remove_file(&caminho).is_err() {
                    continue;
                }
                if apagar {
                    let _ = std::fs::remove_file(&cab);
                }
                retrato.fotos_do_espelho += 1;
                retrato.bytes_das_fotos_do_espelho += bytes;
                continue;
            }
            if !velho(&meta) || (apagar && std::fs::remove_file(&caminho).is_err()) {
                continue;
            }
            retrato.do_espelho += 1;
            retrato.bytes_do_espelho += meta.len();
        }
    }
    retrato
}

/// Se o cabeçalho guardado diz que a resposta é imagem — a segunda linha dele
/// é o tipo (`EspelhoEmDisco::guardar`).
fn e_foto(cab: &Path) -> bool {
    std::fs::read_to_string(cab)
        .ok()
        .and_then(|texto| texto.lines().nth(1).map(|t| t.trim().starts_with("image/")))
        .unwrap_or(false)
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
            FotosLocais::new(self.pool.clone(), self.pasta.path().to_path_buf())
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

    /// 🚨 **O período é a regra** (dono, 17/11/2026, #28): a foto local
    /// parada sai esteja no site ou não — a que não subiu e a da Biblioteca,
    /// sem sessão, também. O app não confere o site.
    #[tokio::test]
    async fn o_periodo_leva_a_foto_esteja_no_site_ou_nao() {
        let c = catalogo().await;
        let sem_subir = c.foto("a", Some("s1"), None, 90).await;
        let biblioteca = c.foto("b", None, None, 90).await;
        let recente = c.foto("c", None, None, 5).await;

        let saiu = c.fotos().limpar_sem_uso_ha(30).await.unwrap();
        assert_eq!(saiu.fotos, 2);
        assert!(!sem_subir.exists() && !biblioteca.exists());
        assert!(!c.existe("a").await && !c.existe("b").await);
        assert!(recente.exists() && c.existe("c").await, "a recente fica");
    }

    /// 🚨 A rejeitada também sai pelo período (#28).
    #[tokio::test]
    async fn a_rejeitada_sai_pelo_periodo() {
        let c = catalogo().await;
        let arquivo = c.foto("a", Some("s1"), Some("site-a"), 90).await;
        sqlx::query("UPDATE photos SET flag = -1 WHERE id = 'a'")
            .execute(&c.pool)
            .await
            .unwrap();

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 1);
        assert!(!arquivo.exists());
    }

    /// 🚨 Só sai arquivo de dentro da pasta do catálogo: a foto trazida do
    /// catálogo do Lightroom aponta para o original do fotógrafo, que não é
    /// cópia do app. Nem o arquivo nem a linha saem.
    #[tokio::test]
    async fn o_arquivo_fora_do_catalogo_fica() {
        let c = catalogo().await;
        let fora = TempDir::new().unwrap();
        let original = fora.path().join("original.jpg");
        std::fs::write(&original, vec![7u8; 1000]).unwrap();
        let quando = (Utc::now() - chrono::Duration::days(90)).to_rfc3339();
        sqlx::query(
            "INSERT INTO photos (id, file_path, is_edited, imported_at, modified_at) \
             VALUES ('lr', ?, 0, ?, ?)",
        )
        .bind(original.to_string_lossy().to_string())
        .bind(&quando)
        .bind(&quando)
        .execute(&c.pool)
        .await
        .unwrap();

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 0);
        assert!(original.exists() && c.existe("lr").await);
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

    /// 🚨 A foto com edição em camadas também sai pelo período (#28).
    #[tokio::test]
    async fn a_foto_com_edicao_em_camadas_sai_pelo_periodo() {
        let c = catalogo().await;
        let arquivo = c.foto("a", Some("s1"), Some("site-a"), 90).await;
        sqlx::query(
            "INSERT INTO edicoes_de_foto (edicao_id, foto_id, diretorio, base_sha256) \
             VALUES ('e1', 'a', '/x', 'h')",
        )
        .execute(&c.pool)
        .await
        .unwrap();

        assert_eq!(c.fotos().limpar_sem_uso_ha(15).await.unwrap().fotos, 1);
        assert!(!arquivo.exists());
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

    /// 🚨 O andamento conta o total no começo e cada foto que sai, com os
    /// bytes — é o que a barra do rodapé desenha (#25).
    #[tokio::test]
    async fn o_andamento_conta_foto_a_foto() {
        let c = catalogo().await;
        c.foto("a", Some("s1"), Some("site-a"), 90).await;
        c.foto("b", Some("s1"), Some("site-b"), 90).await;
        c.foto("c", Some("s2"), None, 90).await;

        let vistos = std::sync::Mutex::new(Vec::new());
        let saiu = c
            .fotos()
            .limpar_sem_uso_ha_contando(30, |a| vistos.lock().unwrap().push(a))
            .await
            .unwrap();

        assert_eq!(saiu.fotos, 3);
        let vistos = vistos.into_inner().unwrap();
        assert_eq!(
            vistos
                .iter()
                .map(|a| (a.feitas, a.total, a.bytes))
                .collect::<Vec<_>>(),
            vec![(0, 3, 0), (1, 3, 1000), (2, 3, 2000), (3, 3, 3000)]
        );
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

        let fotos = c.fotos();
        let antes = fotos.sem_uso_ha(30).await.unwrap();
        assert_eq!((antes.do_espelho, antes.bytes_do_espelho), (1, 500));
        assert!(velho.exists(), "medir não apaga");

        let saiu = fotos.limpar_sem_uso_ha(30).await.unwrap();
        assert_eq!((saiu.do_espelho, saiu.bytes()), (1, 500));
        assert!(!velho.exists() && novo.exists());
    }

    /// Grava no espelho um par corpo + cabeçalho, como `EspelhoEmDisco::guardar`.
    fn guardar_no_espelho(endereco: &Path, nome: &str, tipo: &str, bytes: usize, dias: u64) {
        std::fs::create_dir_all(endereco).unwrap();
        let corpo = endereco.join(format!("{nome}.corpo"));
        let cab = endereco.join(format!("{nome}.cab"));
        std::fs::write(&corpo, vec![1u8; bytes]).unwrap();
        std::fs::write(&cab, format!("1700000000\n{tipo}")).unwrap();
        let quando = SystemTime::now() - Duration::from_secs(dias * 86_400);
        for arquivo in [&corpo, &cab] {
            std::fs::File::options()
                .write(true)
                .open(arquivo)
                .unwrap()
                .set_modified(quando)
                .unwrap();
        }
    }

    /// 🚨 **A foto do site guardada aqui é foto** (#28): a resposta `image/*`
    /// do espelho conta como foto, e não como item do cache, e sai pelo
    /// período junto com o cabeçalho dela. O dado (`json`) segue como item.
    #[tokio::test]
    async fn a_foto_do_espelho_conta_como_foto_e_sai_pelo_periodo() {
        let c = catalogo().await;
        let endereco = c.pasta.path().join("Espelho do site").join("api");
        guardar_no_espelho(&endereco, "foto-velha", "image/jpeg", 700, 40);
        guardar_no_espelho(&endereco, "foto-nova", "image/webp", 600, 2);
        guardar_no_espelho(&endereco, "dado-velho", "application/json", 50, 40);
        let cab_da_foto = endereco.join("foto-velha.cab");
        let tamanho_do_cab = std::fs::metadata(&cab_da_foto).unwrap().len();

        let antes = c.fotos().sem_uso_ha(30).await.unwrap();
        assert_eq!(
            (antes.fotos_do_espelho, antes.bytes_das_fotos_do_espelho),
            (1, 700 + tamanho_do_cab)
        );
        assert_eq!(antes.do_espelho, 2, "o dado: corpo e cabeçalho");
        assert_eq!(antes.todas_as_fotos(), 1);

        let saiu = c.fotos().limpar_sem_uso_ha(30).await.unwrap();
        assert_eq!(saiu.fotos_do_espelho, 1);
        assert!(!endereco.join("foto-velha.corpo").exists() && !cab_da_foto.exists());
        assert!(!endereco.join("dado-velho.corpo").exists());
        assert!(
            endereco.join("foto-nova.corpo").exists() && endereco.join("foto-nova.cab").exists(),
            "a foto lida no período fica"
        );
    }

    /// 📸 O total da aba Armazenamento (#28): toda foto do catálogo com
    /// arquivo aqui dentro e toda foto do espelho, de qualquer idade. O dado do
    /// espelho, a foto sem arquivo e o original de fora do catálogo não contam.
    #[tokio::test]
    async fn o_total_conta_as_fotos_guardadas_de_qualquer_idade() {
        let c = catalogo().await;
        c.foto("velha", Some("s1"), Some("site-a"), 90).await;
        c.foto("nova", None, None, 1).await;
        sqlx::query(
            "INSERT INTO photos (id, file_path, is_edited, imported_at, modified_at) \
             VALUES ('do-site', '', 0, '2026-11-17T00:00:00Z', '2026-11-17T00:00:00Z'), \
                    ('lr', '/fora/do/catalogo.jpg', 0, '2026-11-17T00:00:00Z', '2026-11-17T00:00:00Z')",
        )
        .execute(&c.pool)
        .await
        .unwrap();
        let endereco = c.pasta.path().join("Espelho do site").join("api");
        guardar_no_espelho(&endereco, "foto", "image/jpeg", 700, 0);
        guardar_no_espelho(&endereco, "dado", "application/json", 50, 0);
        let cab = std::fs::metadata(endereco.join("foto.cab")).unwrap().len();

        let total = c.fotos().guardadas().await.unwrap();
        assert_eq!((total.fotos, total.bytes_das_fotos), (2, 2000));
        assert_eq!(
            (total.fotos_do_espelho, total.bytes_das_fotos_do_espelho),
            (1, 700 + cab)
        );
        assert_eq!((total.do_espelho, total.todas_as_fotos()), (0, 3));
        assert!(endereco.join("foto.corpo").exists(), "medir não apaga");
    }
}
