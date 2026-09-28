//! O catálogo das edições em camadas — a tabela `edicoes_de_foto` (migration 026).
//!
//! Guarda a relação foto ↔ projeto ↔ imagem editada vigente, e nenhum pixel
//! (`docs/editor-em-camadas/03-GRAVACAO-E-CATALOGO.md`). É o passo 4 da
//! gravação: a revisão só vale para a Revelação depois de [`CatalogoDeEdicoes::confirmar`].

use std::path::Path;

use editor_core::VersaoEditada;
use sqlx::{Row, SqlitePool};

/// Uma linha do catálogo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdicaoRegistrada {
    pub edicao_id: String,
    pub foto_id: Option<String>,
    pub pos_venda_foto_id: Option<String>,
    /// A pasta do projeto, relativa à raiz do catálogo.
    pub diretorio: String,
    pub revisao: u64,
    /// O nome da imagem editada dentro da pasta.
    pub arquivo: Option<String>,
    pub sha256: Option<String>,
    pub largura: Option<u32>,
    pub altura: Option<u32>,
    pub base_sha256: String,
    /// `false` = o projeto não muda a foto (C30): a Revelação usa o bruto.
    pub ativa: bool,
}

impl EdicaoRegistrada {
    /// A pasta do projeto no disco.
    pub fn pasta(&self, raiz: &Path) -> std::path::PathBuf {
        raiz.join(&self.diretorio)
    }

    /// A versão que a Revelação deve usar — `None` sem imagem editada ativa.
    pub fn versao(&self, raiz: &Path) -> Option<VersaoEditada> {
        if !self.ativa || self.revisao == 0 {
            return None;
        }
        Some(VersaoEditada {
            edicao_id: self.edicao_id.clone(),
            revisao: self.revisao,
            arquivo: self.pasta(raiz).join(self.arquivo.as_ref()?),
            sha256: self.sha256.clone()?,
            largura: self.largura?,
            altura: self.altura?,
            base_sha256: self.base_sha256.clone(),
        })
    }

    /// É desta foto? Pelo id do site ou pelo do catálogo.
    pub fn e_da_foto(&self, foto_id: &str, pos_venda_foto_id: Option<&str>) -> bool {
        pos_venda_foto_id.is_some_and(|pv| self.pos_venda_foto_id.as_deref() == Some(pv))
            || self.foto_id.as_deref() == Some(foto_id)
    }
}

#[derive(Clone)]
pub struct CatalogoDeEdicoes {
    pool: SqlitePool,
}

fn erro(e: sqlx::Error) -> String {
    format!("o catálogo das edições recusou: {e}")
}

impl CatalogoDeEdicoes {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn todas(&self) -> Result<Vec<EdicaoRegistrada>, String> {
        let linhas = sqlx::query(
            "SELECT edicao_id, foto_id, pos_venda_foto_id, diretorio, revisao, arquivo, \
             sha256, largura, altura, base_sha256, ativa FROM edicoes_de_foto",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(erro)?;
        Ok(linhas
            .into_iter()
            .map(|l| EdicaoRegistrada {
                edicao_id: l.get("edicao_id"),
                foto_id: l.get("foto_id"),
                pos_venda_foto_id: l.get("pos_venda_foto_id"),
                diretorio: l.get("diretorio"),
                revisao: l.get::<i64, _>("revisao").max(0) as u64,
                arquivo: l.get("arquivo"),
                sha256: l.get("sha256"),
                largura: l.get::<Option<i64>, _>("largura").map(|v| v as u32),
                altura: l.get::<Option<i64>, _>("altura").map(|v| v as u32),
                base_sha256: l.get("base_sha256"),
                ativa: l.get::<i64, _>("ativa") != 0,
            })
            .collect())
    }

    /// Uma edição nova, ainda sem revisão (a primeira gravação confirma a 1).
    pub async fn criar(&self, edicao: &EdicaoRegistrada) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO edicoes_de_foto \
             (edicao_id, foto_id, pos_venda_foto_id, diretorio, revisao, base_sha256, ativa) \
             VALUES (?, ?, ?, ?, 0, ?, 0)",
        )
        .bind(&edicao.edicao_id)
        .bind(&edicao.foto_id)
        .bind(&edicao.pos_venda_foto_id)
        .bind(&edicao.diretorio)
        .bind(&edicao.base_sha256)
        .execute(&self.pool)
        .await
        .map_err(erro)?;
        Ok(())
    }

    /// O passo 4: a revisão `revisao` passa a valer. `versao` é `None` quando
    /// o projeto não muda a foto (C30).
    ///
    /// 🔑 **Só avança.** Uma confirmação atrasada de uma revisão mais velha (duas
    /// gravações seguidas, a segunda chegando antes) não volta o catálogo.
    pub async fn confirmar(
        &self,
        edicao_id: &str,
        revisao: u64,
        versao: Option<&VersaoEditada>,
    ) -> Result<bool, String> {
        let arquivo = versao.and_then(|v| {
            v.arquivo
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        });
        let feito = sqlx::query(
            "UPDATE edicoes_de_foto SET revisao = ?, arquivo = ?, sha256 = ?, largura = ?, \
             altura = ?, ativa = ?, atualizada_em = CURRENT_TIMESTAMP \
             WHERE edicao_id = ? AND revisao < ?",
        )
        .bind(revisao as i64)
        .bind(arquivo)
        .bind(versao.map(|v| v.sha256.clone()))
        .bind(versao.map(|v| v.largura as i64))
        .bind(versao.map(|v| v.altura as i64))
        .bind(i64::from(versao.is_some()))
        .bind(edicao_id)
        .bind(revisao as i64)
        .execute(&self.pool)
        .await
        .map_err(erro)?;
        Ok(feito.rows_affected() == 1)
    }

    /// A foto editada antes de subir ganhou id no site.
    pub async fn ligar_ao_site(
        &self,
        edicao_id: &str,
        pos_venda_foto_id: &str,
    ) -> Result<(), String> {
        sqlx::query(
            "UPDATE edicoes_de_foto SET pos_venda_foto_id = ? \
             WHERE edicao_id = ? AND pos_venda_foto_id IS NULL",
        )
        .bind(pos_venda_foto_id)
        .bind(edicao_id)
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

    async fn catalogo() -> CatalogoDeEdicoes {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();
        CatalogoDeEdicoes::new(pool)
    }

    fn linha() -> EdicaoRegistrada {
        EdicaoRegistrada {
            edicao_id: "e1".into(),
            foto_id: Some("foto-1".into()),
            pos_venda_foto_id: None,
            diretorio: "edicoes/e1".into(),
            revisao: 0,
            arquivo: None,
            sha256: None,
            largura: None,
            altura: None,
            base_sha256: "b".into(),
            ativa: false,
        }
    }

    fn versao(revisao: u64) -> VersaoEditada {
        VersaoEditada {
            edicao_id: "e1".into(),
            revisao,
            arquivo: format!("/qualquer/edicoes/e1/composta-{revisao}.png").into(),
            sha256: format!("sha{revisao}"),
            largura: 60,
            altura: 40,
            base_sha256: "b".into(),
        }
    }

    #[tokio::test]
    async fn criar_confirmar_e_resolver_a_versao() {
        let c = catalogo().await;
        c.criar(&linha()).await.unwrap();
        let raiz = Path::new("/raiz");
        let criada = &c.todas().await.unwrap()[0];
        assert_eq!(
            criada.versao(raiz),
            None,
            "sem revisão, a Revelação usa o bruto"
        );

        assert!(c.confirmar("e1", 1, Some(&versao(1))).await.unwrap());
        let v = c.todas().await.unwrap()[0].versao(raiz).unwrap();
        assert_eq!(v.revisao, 1);
        assert_eq!(v.arquivo, Path::new("/raiz/edicoes/e1/composta-1.png"));
        assert_eq!((v.largura, v.altura), (60, 40));
    }

    #[tokio::test]
    async fn a_revisao_so_avanca() {
        let c = catalogo().await;
        c.criar(&linha()).await.unwrap();
        c.confirmar("e1", 3, Some(&versao(3))).await.unwrap();
        assert!(!c.confirmar("e1", 2, Some(&versao(2))).await.unwrap());
        assert_eq!(c.todas().await.unwrap()[0].revisao, 3);
    }

    #[tokio::test]
    async fn projeto_sem_efeito_desliga_a_versao() {
        let c = catalogo().await;
        c.criar(&linha()).await.unwrap();
        c.confirmar("e1", 1, Some(&versao(1))).await.unwrap();
        c.confirmar("e1", 2, None).await.unwrap();
        let linha = &c.todas().await.unwrap()[0];
        assert!(!linha.ativa);
        assert_eq!(linha.versao(Path::new("/r")), None);
    }

    #[tokio::test]
    async fn a_foto_que_subiu_e_achada_pelo_id_do_site() {
        let c = catalogo().await;
        c.criar(&linha()).await.unwrap();
        c.ligar_ao_site("e1", "pv-9").await.unwrap();
        let linha = &c.todas().await.unwrap()[0];
        assert!(linha.e_da_foto("site:pv-9", Some("pv-9")));
        assert!(linha.e_da_foto("foto-1", None));
        assert!(!linha.e_da_foto("outra", Some("pv-8")));
    }
}
