//! ⏱️ As sessões da ferramenta de desempenho, no SQLite do catálogo
//! (migration 025).
//!
//! 🔑 **Um lote, uma transação.** A sessão, as métricas e a amostra de quadros
//! entram juntas: uma sessão sem os quadros diria "p95 de 40 ms" sem nada para
//! conferir. Os quadros vão em `INSERT` de várias linhas por vez — mil
//! `INSERT`s soltos numa transação custam dezenas de milissegundos que ninguém
//! precisa pagar.

use async_trait::async_trait;
use domain::desempenho::{
    CabecalhoDaSessao, LinhaDeComparacao, MetricaDeDesempenho, QuadroGravado, SessaoDeDesempenho,
};
use domain::repositories::DesempenhoRepository;
use domain::{DomainError, DomainResult};
use sqlx::sqlite::SqliteRow;
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};

/// Quadros por `INSERT` — 10 colunas × 90 linhas fica abaixo do teto de 999
/// parâmetros dos SQLite antigos.
const QUADROS_POR_INSERT: usize = 90;

#[derive(Clone)]
pub struct SqliteDesempenho {
    pool: SqlitePool,
}

impl SqliteDesempenho {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn erro(e: sqlx::Error) -> DomainError {
    DomainError::InfrastructureError(e.to_string())
}

const COLUNAS_DO_CABECALHO: &str = "id, iniciada_em, terminada_em, duracao_ms, origem, \
    versao_do_app, perfil_de_build, sistema, sistema_versao, arquitetura, cpu, gpu_nome, \
    gpu_backend, gpu_driver, gpu_carimbos, janela_largura_px, janela_altura_px, escala, \
    taxa_do_monitor_hz, taxa_origem, telas, quadros, fps_interacao, mediana_ms, p95_ms, \
    pior_ms, acima_do_orcamento, travamentos, diagnostico, maquina_json, imagens_json, \
    revelacoes_json, travamentos_json";

fn cabecalho(l: &SqliteRow) -> CabecalhoDaSessao {
    CabecalhoDaSessao {
        id: l.get("id"),
        iniciada_em: l.get("iniciada_em"),
        terminada_em: l.get("terminada_em"),
        duracao_ms: l.get("duracao_ms"),
        origem: l.get("origem"),
        versao_do_app: l.get("versao_do_app"),
        perfil_de_build: l.get("perfil_de_build"),
        sistema: l.get("sistema"),
        sistema_versao: l.get("sistema_versao"),
        arquitetura: l.get("arquitetura"),
        cpu: l.get("cpu"),
        gpu_nome: l.get("gpu_nome"),
        gpu_backend: l.get("gpu_backend"),
        gpu_driver: l.get("gpu_driver"),
        gpu_carimbos: l.get::<i64, _>("gpu_carimbos") != 0,
        janela_largura_px: l.get::<i64, _>("janela_largura_px") as u32,
        janela_altura_px: l.get::<i64, _>("janela_altura_px") as u32,
        escala: l.get::<f64, _>("escala") as f32,
        taxa_do_monitor_hz: l.get::<f64, _>("taxa_do_monitor_hz") as f32,
        taxa_origem: l.get("taxa_origem"),
        telas: l.get("telas"),
        quadros: l.get::<i64, _>("quadros") as u64,
        fps_interacao: l.get::<f64, _>("fps_interacao") as f32,
        mediana_ms: l.get::<f64, _>("mediana_ms") as f32,
        p95_ms: l.get::<f64, _>("p95_ms") as f32,
        pior_ms: l.get::<f64, _>("pior_ms") as f32,
        acima_do_orcamento: l.get::<i64, _>("acima_do_orcamento") as u64,
        travamentos: l.get::<i64, _>("travamentos") as u32,
        diagnostico: l.get("diagnostico"),
        maquina_json: l.get("maquina_json"),
        imagens_json: l.get("imagens_json"),
        revelacoes_json: l.get("revelacoes_json"),
        travamentos_json: l.get("travamentos_json"),
    }
}

fn metrica(l: &SqliteRow) -> MetricaDeDesempenho {
    MetricaDeDesempenho {
        operacao: l.get("operacao"),
        etapa: l.get("etapa"),
        onde: l.get("onde"),
        amostras: l.get::<i64, _>("amostras") as u64,
        mediana_ms: l.get::<f64, _>("mediana_ms") as f32,
        p95_ms: l.get::<f64, _>("p95_ms") as f32,
        pior_ms: l.get::<f64, _>("pior_ms") as f32,
        media_ms: l.get::<f64, _>("media_ms") as f32,
        soma_ms: l.get("soma_ms"),
    }
}

#[async_trait]
impl DesempenhoRepository for SqliteDesempenho {
    async fn salvar(&self, sessao: &SessaoDeDesempenho) -> DomainResult<()> {
        let c = &sessao.cabecalho;
        let mut tx = self.pool.begin().await.map_err(erro)?;
        // Regravar a mesma sessão substitui — as filhas saem antes.
        for tabela in [
            "desempenho_quadros",
            "desempenho_metricas",
            "desempenho_sessoes",
        ] {
            let coluna = if tabela == "desempenho_sessoes" {
                "id"
            } else {
                "sessao_id"
            };
            sqlx::query(&format!("DELETE FROM {tabela} WHERE {coluna} = ?"))
                .bind(&c.id)
                .execute(&mut *tx)
                .await
                .map_err(erro)?;
        }
        let mut q: QueryBuilder<Sqlite> = QueryBuilder::new(format!(
            "INSERT INTO desempenho_sessoes ({COLUNAS_DO_CABECALHO}) "
        ));
        q.push_values(std::iter::once(c), |mut b, c| {
            b.push_bind(&c.id)
                .push_bind(&c.iniciada_em)
                .push_bind(&c.terminada_em)
                .push_bind(c.duracao_ms)
                .push_bind(&c.origem)
                .push_bind(&c.versao_do_app)
                .push_bind(&c.perfil_de_build)
                .push_bind(&c.sistema)
                .push_bind(&c.sistema_versao)
                .push_bind(&c.arquitetura)
                .push_bind(&c.cpu)
                .push_bind(&c.gpu_nome)
                .push_bind(&c.gpu_backend)
                .push_bind(&c.gpu_driver)
                .push_bind(c.gpu_carimbos as i64)
                .push_bind(i64::from(c.janela_largura_px))
                .push_bind(i64::from(c.janela_altura_px))
                .push_bind(f64::from(c.escala))
                .push_bind(f64::from(c.taxa_do_monitor_hz))
                .push_bind(&c.taxa_origem)
                .push_bind(&c.telas)
                .push_bind(c.quadros as i64)
                .push_bind(f64::from(c.fps_interacao))
                .push_bind(f64::from(c.mediana_ms))
                .push_bind(f64::from(c.p95_ms))
                .push_bind(f64::from(c.pior_ms))
                .push_bind(c.acima_do_orcamento as i64)
                .push_bind(i64::from(c.travamentos))
                .push_bind(&c.diagnostico)
                .push_bind(&c.maquina_json)
                .push_bind(&c.imagens_json)
                .push_bind(&c.revelacoes_json)
                .push_bind(&c.travamentos_json);
        });
        q.build().execute(&mut *tx).await.map_err(erro)?;

        for lote in sessao.metricas.chunks(QUADROS_POR_INSERT) {
            let mut q: QueryBuilder<Sqlite> = QueryBuilder::new(
                "INSERT INTO desempenho_metricas (sessao_id, operacao, etapa, onde, amostras, \
                 mediana_ms, p95_ms, pior_ms, media_ms, soma_ms) ",
            );
            q.push_values(lote, |mut b, m| {
                b.push_bind(&c.id)
                    .push_bind(&m.operacao)
                    .push_bind(&m.etapa)
                    .push_bind(&m.onde)
                    .push_bind(m.amostras as i64)
                    .push_bind(f64::from(m.mediana_ms))
                    .push_bind(f64::from(m.p95_ms))
                    .push_bind(f64::from(m.pior_ms))
                    .push_bind(f64::from(m.media_ms))
                    .push_bind(m.soma_ms);
            });
            q.build().execute(&mut *tx).await.map_err(erro)?;
        }

        for lote in sessao.quadros.chunks(QUADROS_POR_INSERT) {
            let mut q: QueryBuilder<Sqlite> = QueryBuilder::new(
                "INSERT INTO desempenho_quadros (sessao_id, indice, em_ms, intervalo_ms, \
                 montagem_ms, apresentacao_ms, operacao, tela, lento, etapas_json) ",
            );
            q.push_values(lote, |mut b, f| {
                b.push_bind(&c.id)
                    .push_bind(f.indice as i64)
                    .push_bind(f.em_ms)
                    .push_bind(f64::from(f.intervalo_ms))
                    .push_bind(f64::from(f.montagem_ms))
                    .push_bind(f64::from(f.apresentacao_ms))
                    .push_bind(&f.operacao)
                    .push_bind(&f.tela)
                    .push_bind(f.lento as i64)
                    .push_bind(&f.etapas_json);
            });
            q.build().execute(&mut *tx).await.map_err(erro)?;
        }
        tx.commit().await.map_err(erro)
    }

    async fn listar(&self, limite: u32) -> DomainResult<Vec<CabecalhoDaSessao>> {
        let linhas = sqlx::query(&format!(
            "SELECT {COLUNAS_DO_CABECALHO} FROM desempenho_sessoes \
             ORDER BY iniciada_em DESC LIMIT ?"
        ))
        .bind(i64::from(limite))
        .fetch_all(&self.pool)
        .await
        .map_err(erro)?;
        Ok(linhas.iter().map(cabecalho).collect())
    }

    async fn carregar(&self, id: &str) -> DomainResult<Option<SessaoDeDesempenho>> {
        let Some(linha) = sqlx::query(&format!(
            "SELECT {COLUNAS_DO_CABECALHO} FROM desempenho_sessoes WHERE id = ?"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(erro)?
        else {
            return Ok(None);
        };
        let metricas = sqlx::query(
            "SELECT * FROM desempenho_metricas WHERE sessao_id = ? ORDER BY operacao, etapa",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(erro)?;
        let quadros =
            sqlx::query("SELECT * FROM desempenho_quadros WHERE sessao_id = ? ORDER BY indice")
                .bind(id)
                .fetch_all(&self.pool)
                .await
                .map_err(erro)?;
        Ok(Some(SessaoDeDesempenho {
            cabecalho: cabecalho(&linha),
            metricas: metricas.iter().map(metrica).collect(),
            quadros: quadros
                .iter()
                .map(|l| QuadroGravado {
                    indice: l.get::<i64, _>("indice") as u64,
                    em_ms: l.get("em_ms"),
                    intervalo_ms: l.get::<f64, _>("intervalo_ms") as f32,
                    montagem_ms: l.get::<f64, _>("montagem_ms") as f32,
                    apresentacao_ms: l.get::<f64, _>("apresentacao_ms") as f32,
                    operacao: l.get("operacao"),
                    tela: l.get("tela"),
                    lento: l.get::<i64, _>("lento") != 0,
                    etapas_json: l.get("etapas_json"),
                })
                .collect(),
        }))
    }

    async fn comparar(&self, operacao: &str) -> DomainResult<Vec<LinhaDeComparacao>> {
        let linhas = sqlx::query(
            "SELECT s.id AS sessao_id, s.iniciada_em, s.sistema, s.gpu_nome, s.gpu_backend, \
                    s.taxa_do_monitor_hz, m.* \
             FROM desempenho_metricas m JOIN desempenho_sessoes s ON s.id = m.sessao_id \
             WHERE m.operacao = ? \
             ORDER BY s.sistema, s.iniciada_em DESC, m.etapa",
        )
        .bind(operacao)
        .fetch_all(&self.pool)
        .await
        .map_err(erro)?;
        Ok(linhas
            .iter()
            .map(|l| LinhaDeComparacao {
                sessao_id: l.get("sessao_id"),
                iniciada_em: l.get("iniciada_em"),
                sistema: l.get("sistema"),
                gpu_nome: l.get("gpu_nome"),
                gpu_backend: l.get("gpu_backend"),
                taxa_do_monitor_hz: l.get::<f64, _>("taxa_do_monitor_hz") as f32,
                metrica: metrica(l),
            })
            .collect())
    }

    async fn apagar(&self, id: &str) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(erro)?;
        for (tabela, coluna) in [
            ("desempenho_quadros", "sessao_id"),
            ("desempenho_metricas", "sessao_id"),
            ("desempenho_sessoes", "id"),
        ] {
            sqlx::query(&format!("DELETE FROM {tabela} WHERE {coluna} = ?"))
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(erro)?;
        }
        tx.commit().await.map_err(erro)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    async fn deposito() -> SqliteDesempenho {
        let pool = crate::database::create_pool("sqlite::memory:")
            .await
            .expect("abrir o banco em memória");
        crate::database::run_migrations(&pool)
            .await
            .expect("as migrations rodam");
        SqliteDesempenho::new(pool)
    }

    fn sessao(id: &str, sistema: &str, quadros: usize) -> SessaoDeDesempenho {
        SessaoDeDesempenho {
            cabecalho: CabecalhoDaSessao {
                id: id.into(),
                iniciada_em: format!("2026-09-27T12:00:0{}Z", id.len() % 10),
                sistema: sistema.into(),
                gpu_nome: "GPU de teste".into(),
                quadros: quadros as u64,
                p95_ms: 22.5,
                maquina_json: "{\"cpu\":\"x\"}".into(),
                ..Default::default()
            },
            metricas: vec![
                MetricaDeDesempenho {
                    operacao: "arrasto_de_slider".into(),
                    etapa: "intervalo_do_quadro".into(),
                    onde: "quadro".into(),
                    amostras: 300,
                    mediana_ms: 16.7,
                    p95_ms: 33.4,
                    ..Default::default()
                },
                MetricaDeDesempenho {
                    operacao: "rolagem".into(),
                    etapa: "intervalo_do_quadro".into(),
                    onde: "quadro".into(),
                    amostras: 100,
                    ..Default::default()
                },
            ],
            quadros: (0..quadros)
                .map(|i| QuadroGravado {
                    indice: i as u64,
                    em_ms: i as f64 * 16.7,
                    intervalo_ms: 16.7,
                    operacao: "arrasto_de_slider".into(),
                    tela: "revelacao".into(),
                    lento: i % 50 == 0,
                    etapas_json: "{}".into(),
                    ..Default::default()
                })
                .collect(),
        }
    }

    /// 🔑 O ciclo inteiro contra SQLite de verdade, com a migration — e com
    /// quadros acima do teto de parâmetros de um `INSERT` só.
    #[tokio::test]
    async fn salva_lista_carrega_compara_e_apaga() {
        let d = deposito().await;
        let windows = sessao("s-win", "windows", 1_000);
        d.salvar(&windows).await.unwrap();
        d.salvar(&sessao("s-linux", "linux", 10)).await.unwrap();

        let lista = d.listar(10).await.unwrap();
        assert_eq!(lista.len(), 2);

        let lida = d.carregar("s-win").await.unwrap().expect("gravada");
        assert_eq!(lida, windows, "volta igual ao que foi");

        let linhas = d.comparar("arrasto_de_slider").await.unwrap();
        let sistemas: Vec<_> = linhas.iter().map(|l| l.sistema.as_str()).collect();
        assert_eq!(sistemas, vec!["linux", "windows"]);
        assert_eq!(linhas[1].metrica.p95_ms, 33.4);

        // Regravar substitui, sem duplicar filhas.
        d.salvar(&windows).await.unwrap();
        assert_eq!(d.comparar("rolagem").await.unwrap().len(), 2);

        d.apagar("s-win").await.unwrap();
        assert!(d.carregar("s-win").await.unwrap().is_none());
        assert_eq!(d.comparar("arrasto_de_slider").await.unwrap().len(), 1);
    }
}
