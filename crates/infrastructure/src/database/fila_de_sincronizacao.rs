//! 📴 A fila de sincronização — a tabela `fila_de_sincronizacao` (migration
//! 029). O que cada gesto é, e como ele vai ao site, é do ui-gpui
//! (`sincronizacao/`); aqui ele é só texto, guardado na ordem em que aconteceu.
//!
//! 🔑 **O mesmo SQLite do catálogo**: o gesto feito sem rede sobrevive a fechar
//! o app, a desligar a máquina e a abrir de novo no dia seguinte, junto com as
//! fotos e as notas que ele descreve.

use sqlx::{Row, SqlitePool};

/// Um gesto guardado.
#[derive(Debug, Clone, PartialEq)]
pub struct GestoGuardado {
    /// A ordem: a fila sai por ele.
    pub id: i64,
    /// O UUID do gesto, dado na tela.
    pub chave: String,
    pub tipo: String,
    /// O que o gesto mexe — a foto, a sessão, a rota do caixa.
    pub alvo: String,
    /// A sessão do gesto, quando há uma — é por ela que a sessão criada sem
    /// rede troca de id, se o site escolher outro.
    pub galeria_id: Option<String>,
    pub corpo: serde_json::Value,
    /// A hora do balcão, RFC 3339.
    pub ocorrido_em: String,
    pub tentativas: i64,
    pub estado: EstadoDoGesto,
    /// A frase do site, no conflito.
    pub erro: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDoGesto {
    /// Espera a vez, e a rede.
    Pendente,
    /// O site recusou por regra: espera o operador.
    Conflito,
}

impl EstadoDoGesto {
    fn texto(self) -> &'static str {
        match self {
            Self::Pendente => "pendente",
            Self::Conflito => "conflito",
        }
    }

    fn do_texto(texto: &str) -> Self {
        match texto {
            "conflito" => Self::Conflito,
            _ => Self::Pendente,
        }
    }
}

/// Um gesto novo, antes de ganhar a ordem.
#[derive(Debug, Clone, PartialEq)]
pub struct GestoNovo {
    pub chave: String,
    pub tipo: String,
    pub alvo: String,
    pub galeria_id: Option<String>,
    pub corpo: serde_json::Value,
    pub ocorrido_em: String,
}

#[derive(Clone)]
pub struct FilaDeSincronizacao {
    pool: SqlitePool,
}

fn erro(e: sqlx::Error) -> String {
    format!("a fila de sincronização recusou: {e}")
}

fn do_banco(linha: &sqlx::sqlite::SqliteRow) -> GestoGuardado {
    let corpo: String = linha.get("corpo");
    GestoGuardado {
        id: linha.get("id"),
        chave: linha.get("chave"),
        tipo: linha.get("tipo"),
        alvo: linha.get("alvo"),
        galeria_id: linha.get("galeria_id"),
        corpo: serde_json::from_str(&corpo).unwrap_or(serde_json::Value::Null),
        ocorrido_em: linha.get("ocorrido_em"),
        tentativas: linha.get("tentativas"),
        estado: EstadoDoGesto::do_texto(&linha.get::<String, _>("estado")),
        erro: linha.get("erro"),
    }
}

const COLUNAS: &str =
    "id, chave, tipo, alvo, galeria_id, corpo, ocorrido_em, tentativas, estado, erro";

impl FilaDeSincronizacao {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Guarda o gesto no fim da fila e devolve a ordem dele.
    pub async fn enfileirar(&self, gesto: &GestoNovo) -> Result<i64, String> {
        let resultado = sqlx::query(
            "INSERT INTO fila_de_sincronizacao (chave, tipo, alvo, galeria_id, corpo, ocorrido_em) \
             VALUES (?, ?, ?, ?, ?, ?) ON CONFLICT(chave) DO NOTHING",
        )
        .bind(&gesto.chave)
        .bind(&gesto.tipo)
        .bind(&gesto.alvo)
        .bind(&gesto.galeria_id)
        .bind(gesto.corpo.to_string())
        .bind(&gesto.ocorrido_em)
        .execute(&self.pool)
        .await
        .map_err(erro)?;
        Ok(resultado.last_insert_rowid())
    }

    /// Os que esperam a vez, na ordem.
    pub async fn pendentes(&self) -> Result<Vec<GestoGuardado>, String> {
        self.no_estado(EstadoDoGesto::Pendente).await
    }

    /// Os primeiros `limite` que esperam a vez — o que o trabalhador leva
    /// agora.
    ///
    /// 🚨 **Só o começo da fila.** Ler todos a cada gesto era ler (e desfazer o
    /// JSON de) duzentas linhas para mandar uma — e duzentas vezes, numa
    /// sessão inteira guardada sem rede.
    pub async fn proximas(&self, limite: u32) -> Result<Vec<GestoGuardado>, String> {
        let linhas = sqlx::query(&format!(
            "SELECT {COLUNAS} FROM fila_de_sincronizacao WHERE estado = 'pendente' \
             ORDER BY id LIMIT ?"
        ))
        .bind(limite)
        .fetch_all(&self.pool)
        .await
        .map_err(erro)?;
        Ok(linhas.iter().map(do_banco).collect())
    }

    /// Os que o site recusou, na ordem.
    pub async fn conflitos(&self) -> Result<Vec<GestoGuardado>, String> {
        self.no_estado(EstadoDoGesto::Conflito).await
    }

    /// Todos, pendentes e em conflito, na ordem — o que a tela soma por cima
    /// do espelho.
    pub async fn todos(&self) -> Result<Vec<GestoGuardado>, String> {
        let linhas = sqlx::query(&format!(
            "SELECT {COLUNAS} FROM fila_de_sincronizacao ORDER BY id"
        ))
        .fetch_all(&self.pool)
        .await
        .map_err(erro)?;
        Ok(linhas.iter().map(do_banco).collect())
    }

    async fn no_estado(&self, estado: EstadoDoGesto) -> Result<Vec<GestoGuardado>, String> {
        let linhas = sqlx::query(&format!(
            "SELECT {COLUNAS} FROM fila_de_sincronizacao WHERE estado = ? ORDER BY id"
        ))
        .bind(estado.texto())
        .fetch_all(&self.pool)
        .await
        .map_err(erro)?;
        Ok(linhas.iter().map(do_banco).collect())
    }

    /// Quantos pendentes e quantos em conflito — o selo do rodapé.
    pub async fn contar(&self) -> Result<(usize, usize), String> {
        let linha = sqlx::query(
            "SELECT \
               SUM(CASE WHEN estado = 'pendente' THEN 1 ELSE 0 END) AS pendentes, \
               SUM(CASE WHEN estado = 'conflito' THEN 1 ELSE 0 END) AS conflitos \
             FROM fila_de_sincronizacao",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(erro)?;
        let pendentes: Option<i64> = linha.get("pendentes");
        let conflitos: Option<i64> = linha.get("conflitos");
        Ok((
            pendentes.unwrap_or(0) as usize,
            conflitos.unwrap_or(0) as usize,
        ))
    }

    /// Chegou ao site: sai da fila.
    pub async fn concluir(&self, id: i64) -> Result<(), String> {
        sqlx::query("DELETE FROM fila_de_sincronizacao WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(erro)?;
        Ok(())
    }

    /// O operador desistiu do gesto — o mesmo `DELETE`, com outro nome na
    /// tela.
    pub async fn descartar(&self, id: i64) -> Result<(), String> {
        self.concluir(id).await
    }

    /// Mais uma tentativa que não chegou (sem rede): o gesto fica na vez.
    pub async fn tentou(&self, id: i64) -> Result<(), String> {
        sqlx::query("UPDATE fila_de_sincronizacao SET tentativas = tentativas + 1 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(erro)?;
        Ok(())
    }

    /// O site recusou por regra: o gesto sai da vez e espera o operador.
    pub async fn conflito(&self, id: i64, frase: &str) -> Result<(), String> {
        sqlx::query(
            "UPDATE fila_de_sincronizacao SET estado = 'conflito', erro = ?, \
             tentativas = tentativas + 1 WHERE id = ?",
        )
        .bind(frase)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(erro)?;
        Ok(())
    }

    /// O operador pediu para tentar de novo o gesto recusado.
    pub async fn refazer(&self, id: i64) -> Result<(), String> {
        sqlx::query(
            "UPDATE fila_de_sincronizacao SET estado = 'pendente', erro = NULL WHERE id = ?",
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(erro)?;
        Ok(())
    }

    /// 🔁 A sessão criada sem rede ganhou outro id no site: todo gesto que a
    /// cita passa a citar o de lá — na coluna, no alvo e dentro do corpo (a
    /// rota do caixa, `galeria_id` da venda). Um id é um UUID, e trocar o
    /// texto dele não acerta outra coisa por acaso.
    pub async fn trocar_galeria(&self, de: &str, para: &str) -> Result<(), String> {
        if de == para || de.is_empty() {
            return Ok(());
        }
        sqlx::query(
            "UPDATE fila_de_sincronizacao SET \
               galeria_id = CASE WHEN galeria_id = ?1 THEN ?2 ELSE galeria_id END, \
               alvo = REPLACE(alvo, ?1, ?2), \
               corpo = REPLACE(corpo, ?1, ?2) \
             WHERE galeria_id = ?1 OR alvo LIKE '%' || ?1 || '%' OR corpo LIKE '%' || ?1 || '%'",
        )
        .bind(de)
        .bind(para)
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
    use serde_json::json;

    async fn fila() -> FilaDeSincronizacao {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();
        FilaDeSincronizacao::new(pool)
    }

    fn gesto(tipo: &str, alvo: &str, galeria: Option<&str>, corpo: serde_json::Value) -> GestoNovo {
        GestoNovo {
            chave: uuid::Uuid::new_v4().to_string(),
            tipo: tipo.into(),
            alvo: alvo.into(),
            galeria_id: galeria.map(str::to_string),
            corpo,
            ocorrido_em: "2026-10-20T14:00:00-03:00".into(),
        }
    }

    #[tokio::test]
    async fn sai_na_ordem_e_o_concluido_some() {
        let f = fila().await;
        let a = f
            .enfileirar(&gesto(
                "criar_galeria",
                "g1",
                Some("g1"),
                json!({"titulo": "Ana"}),
            ))
            .await
            .unwrap();
        let b = f
            .enfileirar(&gesto("mudar_foto", "f1", Some("g1"), json!({"nota": 5})))
            .await
            .unwrap();
        assert!(a < b);
        let pendentes = f.pendentes().await.unwrap();
        assert_eq!(
            pendentes.iter().map(|g| g.id).collect::<Vec<_>>(),
            vec![a, b]
        );
        assert_eq!(pendentes[1].corpo, json!({"nota": 5}));
        assert_eq!(f.contar().await.unwrap(), (2, 0));

        assert_eq!(
            f.proximas(1)
                .await
                .unwrap()
                .iter()
                .map(|g| g.id)
                .collect::<Vec<_>>(),
            vec![a]
        );
        f.tentou(a).await.unwrap();
        assert_eq!(f.pendentes().await.unwrap()[0].tentativas, 1);

        f.concluir(a).await.unwrap();
        assert_eq!(f.pendentes().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn o_conflito_sai_da_vez_e_volta_ao_refazer() {
        let f = fila().await;
        let id = f
            .enfileirar(&gesto(
                "mudar_foto",
                "f1",
                None,
                json!({"estado": "levada_no_balcao"}),
            ))
            .await
            .unwrap();
        f.conflito(id, "a foto já foi comprada").await.unwrap();
        assert!(f.pendentes().await.unwrap().is_empty());
        let conflitos = f.conflitos().await.unwrap();
        assert_eq!(conflitos[0].erro.as_deref(), Some("a foto já foi comprada"));
        assert_eq!(conflitos[0].estado, EstadoDoGesto::Conflito);
        assert_eq!(f.contar().await.unwrap(), (0, 1));

        f.refazer(id).await.unwrap();
        assert_eq!(f.pendentes().await.unwrap()[0].erro, None);
        f.descartar(id).await.unwrap();
        assert_eq!(f.contar().await.unwrap(), (0, 0));
    }

    #[tokio::test]
    async fn a_sessao_que_ganhou_outro_id_leva_os_gestos_junto() {
        let f = fila().await;
        let local = "11111111-1111-4111-8111-111111111111";
        f.enfileirar(&gesto(
            "criar_galeria",
            local,
            Some(local),
            json!({"id": local}),
        ))
        .await
        .unwrap();
        f.enfileirar(&gesto(
            "pedido_json",
            &format!("POST /pos-venda/caixa/galerias/{local}/vendas"),
            Some(local),
            json!({"caminho": format!("/pos-venda/caixa/galerias/{local}/vendas")}),
        ))
        .await
        .unwrap();
        f.enfileirar(&gesto("mudar_foto", "f9", Some("outra"), json!({})))
            .await
            .unwrap();

        f.trocar_galeria(local, "site-42").await.unwrap();
        let todos = f.todos().await.unwrap();
        assert_eq!(todos[0].galeria_id.as_deref(), Some("site-42"));
        assert_eq!(
            todos[1].alvo,
            "POST /pos-venda/caixa/galerias/site-42/vendas"
        );
        assert_eq!(
            todos[1].corpo,
            json!({"caminho": "/pos-venda/caixa/galerias/site-42/vendas"})
        );
        assert_eq!(todos[2].galeria_id.as_deref(), Some("outra"));
    }
}
