//! A área temporária da importação no catálogo (etapa D, passo 2; G4, G11, G12).
//!
//! É o `importacao/deposito.ts` do site com o SQLite atrás. A página e os
//! workers falam com ela pelo esquema `vlb://` (`protocolo.rs`), e o formato do
//! item é o do site: um objeto JSON, mais três arquivos (`arquivo`,
//! `arquivoBruto`, `previa`) que moram em `importacao/<id>/`.
//!
//! # As regras
//!
//! - **Uma gravação é uma transação.** Os arquivos novos vão para o disco
//!   antes; a linha muda depois, de uma vez; os arquivos substituídos só saem
//!   depois do `COMMIT`. Uma falha no meio apaga os novos e não toca nos velhos.
//! - **O BRUTO nasce uma vez (G4, C2).** Com o bruto guardado, a gravação que
//!   tenta trocá-lo (ou a impressão digital dele) é ignorada nessa parte, como o
//!   `guardarOBruto` do site; o gatilho da migration é a segunda trava. O
//!   SHA-256 é calculado aqui, dos bytes que chegaram.
//! - **Quem grava diz de que versão partiu.** Com `versao` informada e
//!   diferente da atual, nada muda e a resposta é [`Gravado::Conflito`]: quem
//!   gravou leu antes de outro gravar, e precisa ler de novo.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{Catalogo, ErroDoCatalogo};

pub const PASTA: &str = "importacao";

/// As três partes com bytes, pelo nome do campo no item do site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Qual {
    Arquivo,
    ArquivoBruto,
    Previa,
}

impl Qual {
    pub const TODAS: [Qual; 3] = [Qual::Arquivo, Qual::ArquivoBruto, Qual::Previa];

    pub fn do_caminho(texto: &str) -> Option<Qual> {
        match texto {
            "arquivo" => Some(Qual::Arquivo),
            "arquivoBruto" => Some(Qual::ArquivoBruto),
            "previa" => Some(Qual::Previa),
            _ => None,
        }
    }

    fn coluna(self) -> &'static str {
        match self {
            Qual::Arquivo => "arquivo",
            Qual::ArquivoBruto => "bruto",
            Qual::Previa => "previa",
        }
    }
}

/// Uma parte nova, como chegou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bytes {
    pub tipo: String,
    pub nome: Option<String>,
    pub bytes: Vec<u8>,
}

/// O que fazer com uma parte.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Parte {
    /// Não veio na gravação: fica como está.
    #[default]
    Manter,
    /// Veio `null`: sai.
    Apagar,
    Nova(Bytes),
}

/// Uma gravação de um item.
#[derive(Debug, Clone, Default)]
pub struct Gravacao {
    pub id: String,
    /// `true` substitui os campos do item inteiro (o `put` do IndexedDB);
    /// `false` junta os campos ao que está lá. As partes seguem [`Parte`] nos
    /// dois casos.
    pub substituir: bool,
    pub campos: Map<String, Value>,
    /// Campos que a gravação tira (os que eram `undefined` no JavaScript).
    pub remover: Vec<String>,
    pub partes: BTreeMap<String, Parte>,
    /// A versão de que quem grava partiu, quando ele leu antes.
    pub versao: Option<i64>,
}

/// O que uma gravação fez.
#[derive(Debug, Clone, PartialEq)]
pub enum Gravado {
    Pronto(Box<Item>),
    /// A versão mudou desde a leitura; nada foi gravado.
    Conflito {
        id: String,
    },
    /// Um `juntar` num item que não existe; nada foi gravado.
    Ausente {
        id: String,
    },
}

/// Uma parte guardada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Guardada {
    pub tipo: String,
    pub nome: Option<String>,
    pub bytes: i64,
    #[serde(skip)]
    pub caminho: String,
}

/// O item como a página o lê: os campos, a versão e o que há de cada parte.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Item {
    pub campos: Map<String, Value>,
    pub versao: i64,
    pub arquivo: Option<Guardada>,
    #[serde(rename = "arquivoBruto")]
    pub bruto: Option<Guardada>,
    pub previa: Option<Guardada>,
}

impl Item {
    pub fn parte(&self, qual: Qual) -> Option<&Guardada> {
        match qual {
            Qual::Arquivo => self.arquivo.as_ref(),
            Qual::ArquivoBruto => self.bruto.as_ref(),
            Qual::Previa => self.previa.as_ref(),
        }
    }
}

/// Um id que vira nome de pasta sem surpresa.
fn id_seguro(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':'))
}

/// A pasta do item. `:` vira `_`, porque o Windows não o aceita em nome.
fn pasta_do_item(id: &str) -> PathBuf {
    PathBuf::from(PASTA).join(id.replace(':', "_"))
}

const CAMPOS_DA_LINHA: &str = "item, versao, \
    arquivo, arquivo_tipo, arquivo_nome, arquivo_bytes, \
    bruto, bruto_tipo, bruto_nome, bruto_bytes, \
    previa, previa_tipo, previa_nome, previa_bytes";

fn item_da_linha(l: &rusqlite::Row<'_>) -> rusqlite::Result<Item> {
    let parte = |i: usize| -> rusqlite::Result<Option<Guardada>> {
        let caminho: Option<String> = l.get(i)?;
        Ok(match caminho {
            Some(caminho) => Some(Guardada {
                caminho,
                tipo: l.get::<_, Option<String>>(i + 1)?.unwrap_or_default(),
                nome: l.get(i + 2)?,
                bytes: l.get::<_, Option<i64>>(i + 3)?.unwrap_or(0),
            }),
            None => None,
        })
    };
    let texto: String = l.get(0)?;
    let campos = match serde_json::from_str::<Value>(&texto) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    };
    Ok(Item {
        campos,
        versao: l.get(1)?,
        arquivo: parte(2)?,
        bruto: parte(6)?,
        previa: parte(10)?,
    })
}

fn ler_na(t: &rusqlite::Connection, id: &str) -> Result<Option<Item>, ErroDoCatalogo> {
    Ok(t.query_row(
        &format!("SELECT {CAMPOS_DA_LINHA} FROM importacao WHERE id = ?1"),
        [id],
        item_da_linha,
    )
    .optional()?)
}

fn texto(campos: &Map<String, Value>, nome: &str) -> Option<String> {
    campos.get(nome).and_then(Value::as_str).map(str::to_string)
}

fn numero(campos: &Map<String, Value>, nome: &str) -> i64 {
    campos
        .get(nome)
        .and_then(|v| v.as_f64())
        .map(|n| n as i64)
        .unwrap_or(0)
}

/// Um arquivo gravado nesta operação, para desfazer se a transação falhar.
struct Escrito(PathBuf);

impl Catalogo {
    /// Um item, sem os bytes.
    pub fn importacao_item(&self, id: &str) -> Result<Option<Item>, ErroDoCatalogo> {
        if !id_seguro(id) {
            return Ok(None);
        }
        ler_na(&self.banco, id)
    }

    /// Os itens de uma galeria (ou todos), na ordem da fila do site.
    pub fn importacao_itens(&self, galeria: Option<&str>) -> Result<Vec<Item>, ErroDoCatalogo> {
        let ordem = "ORDER BY criado_em, ordem";
        let mut consulta = match galeria {
            Some(_) => self.banco.prepare(&format!(
                "SELECT {CAMPOS_DA_LINHA} FROM importacao WHERE galeria_id = ?1 {ordem}"
            ))?,
            None => self
                .banco
                .prepare(&format!("SELECT {CAMPOS_DA_LINHA} FROM importacao {ordem}"))?,
        };
        let linhas = match galeria {
            Some(g) => consulta.query_map([g], item_da_linha)?,
            None => consulta.query_map([], item_da_linha)?,
        };
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    /// O caminho absoluto dos bytes de uma parte.
    pub fn importacao_arquivo(
        &self,
        id: &str,
        qual: Qual,
    ) -> Result<Option<(PathBuf, Guardada)>, ErroDoCatalogo> {
        let Some(item) = self.importacao_item(id)? else {
            return Ok(None);
        };
        Ok(item
            .parte(qual)
            .cloned()
            .map(|g| (self.raiz.join(&g.caminho), g)))
    }

    /// Várias gravações numa transação só: ou todas, ou nenhuma.
    ///
    /// Um conflito ou um item ausente numa delas não desfaz as outras: ele só
    /// volta na lista, para quem gravou ler de novo.
    pub fn importacao_gravar(
        &mut self,
        gravacoes: Vec<Gravacao>,
        agora: &str,
    ) -> Result<Vec<Gravado>, ErroDoCatalogo> {
        for g in &gravacoes {
            if !id_seguro(&g.id) {
                return Err(ErroDoCatalogo::Entrada(format!(
                    "id de item inválido: {}",
                    g.id
                )));
            }
        }
        let raiz = self.raiz.clone();
        let mut escritos: Vec<Escrito> = Vec::new();
        let mut substituidos: Vec<PathBuf> = Vec::new();
        let resultado = (|| {
            let t = self.banco.transaction()?;
            let mut feitos = Vec::with_capacity(gravacoes.len());
            for g in gravacoes {
                feitos.push(gravar_um(
                    &t,
                    &raiz,
                    g,
                    agora,
                    &mut escritos,
                    &mut substituidos,
                )?);
            }
            t.commit()?;
            Ok::<_, ErroDoCatalogo>(feitos)
        })();
        match resultado {
            Ok(feitos) => {
                for velho in substituidos {
                    let _ = std::fs::remove_file(velho);
                }
                Ok(feitos)
            }
            Err(erro) => {
                for Escrito(novo) in escritos {
                    let _ = std::fs::remove_file(novo);
                }
                Err(erro)
            }
        }
    }

    /// Tira um item e os bytes dele. Só o operador pede isto (o site garante).
    pub fn importacao_apagar(&mut self, id: &str) -> Result<bool, ErroDoCatalogo> {
        if !id_seguro(id) {
            return Ok(false);
        }
        let apagadas = self
            .banco
            .execute("DELETE FROM importacao WHERE id = ?1", [id])?;
        if apagadas > 0 {
            let _ = std::fs::remove_dir_all(self.raiz.join(pasta_do_item(id)));
        }
        Ok(apagadas > 0)
    }

    /// Passa os itens de uma galeria para outra, numa transação.
    pub fn importacao_trocar_galeria(
        &mut self,
        de: &str,
        para: &str,
        agora: &str,
    ) -> Result<Vec<Item>, ErroDoCatalogo> {
        if de.is_empty() || para.is_empty() || de == para {
            return Ok(Vec::new());
        }
        let t = self.banco.transaction()?;
        let ids: Vec<String> = {
            let mut c = t.prepare("SELECT id FROM importacao WHERE galeria_id = ?1")?;
            let ids = c.query_map([de], |l| l.get(0))?.collect::<Result<_, _>>()?;
            ids
        };
        for id in &ids {
            let atual = ler_na(&t, id)?.expect("o item acabou de ser lido");
            let mut campos = atual.campos;
            campos.insert("galeriaId".into(), Value::String(para.to_string()));
            t.execute(
                "UPDATE importacao SET galeria_id = ?2, item = ?3, versao = versao + 1,
                 atualizada_em = ?4 WHERE id = ?1",
                params![id, para, Value::Object(campos).to_string(), agora],
            )?;
        }
        t.commit()?;
        self.importacao_itens(Some(para))
    }

    /// Apaga todos os itens de uma galeria. Devolve os ids.
    pub fn importacao_apagar_galeria(
        &mut self,
        galeria: &str,
    ) -> Result<Vec<String>, ErroDoCatalogo> {
        let ids: Vec<String> = {
            let mut c = self
                .banco
                .prepare("SELECT id FROM importacao WHERE galeria_id = ?1")?;
            let ids = c
                .query_map([galeria], |l| l.get(0))?
                .collect::<Result<_, _>>()?;
            ids
        };
        self.banco
            .execute("DELETE FROM importacao WHERE galeria_id = ?1", [galeria])?;
        for id in &ids {
            let _ = std::fs::remove_dir_all(self.raiz.join(pasta_do_item(id)));
        }
        Ok(ids)
    }
}

fn gravar_um(
    t: &Transaction<'_>,
    raiz: &Path,
    g: Gravacao,
    agora: &str,
    escritos: &mut Vec<Escrito>,
    substituidos: &mut Vec<PathBuf>,
) -> Result<Gravado, ErroDoCatalogo> {
    let atual = ler_na(t, &g.id)?;
    if let (Some(esperada), Some(atual)) = (g.versao, &atual) {
        if esperada != atual.versao {
            return Ok(Gravado::Conflito { id: g.id });
        }
    }
    if atual.is_none() && !g.substituir {
        return Ok(Gravado::Ausente { id: g.id });
    }

    // Os campos: o item inteiro, ou o de agora com os que vieram por cima.
    let mut campos = match (&atual, g.substituir) {
        (Some(a), false) => a.campos.clone(),
        _ => Map::new(),
    };
    for (chave, valor) in g.campos {
        campos.insert(chave, valor);
    }
    for chave in &g.remover {
        campos.remove(chave);
    }
    // Os bytes nunca moram no JSON.
    for qual in Qual::TODAS {
        campos.remove(match qual {
            Qual::Arquivo => "arquivo",
            Qual::ArquivoBruto => "arquivoBruto",
            Qual::Previa => "previa",
        });
    }
    campos.insert("id".into(), Value::String(g.id.clone()));

    let bruto_guardado = atual.as_ref().and_then(|a| a.bruto.clone());
    if bruto_guardado.is_some() {
        // G4: com o bruto guardado, a impressão digital é a dele.
        let de_antes = atual
            .as_ref()
            .and_then(|a| a.campos.get("hashDoBruto").cloned())
            .unwrap_or(Value::Null);
        campos.insert("hashDoBruto".into(), de_antes);
    }

    let pasta = pasta_do_item(&g.id);
    let versao = atual.as_ref().map(|a| a.versao + 1).unwrap_or(1);
    let mut novas: BTreeMap<&'static str, Option<(Guardada, Option<String>)>> = BTreeMap::new();
    for qual in Qual::TODAS {
        let nome_do_campo = match qual {
            Qual::Arquivo => "arquivo",
            Qual::ArquivoBruto => "arquivoBruto",
            Qual::Previa => "previa",
        };
        let pedida = g.partes.get(nome_do_campo).cloned().unwrap_or_default();
        let antes = atual.as_ref().and_then(|a| a.parte(qual).cloned());
        // 🚨 Parte ausente fica, mesmo substituindo o item: quem grava de volta
        // um item lido da lista não tem os bytes na mão, e os omite. Só `null`
        // tira.
        if qual == Qual::ArquivoBruto && bruto_guardado.is_some() {
            // G4: o bruto guardado fica, peça-se o que se pedir.
            continue;
        }
        match pedida {
            Parte::Manter => {}
            Parte::Apagar => {
                if let Some(antes) = antes {
                    substituidos.push(raiz.join(&antes.caminho));
                }
                novas.insert(qual.coluna(), None);
            }
            Parte::Nova(b) => {
                let relativo = pasta.join(format!("{}-v{versao}.bin", qual.coluna()));
                let absoluto = raiz.join(&relativo);
                if let Some(pai) = absoluto.parent() {
                    std::fs::create_dir_all(pai)
                        .map_err(|e| ErroDoCatalogo::Pasta(pai.to_path_buf(), e))?;
                }
                std::fs::write(&absoluto, &b.bytes)
                    .map_err(|e| ErroDoCatalogo::Pasta(absoluto.clone(), e))?;
                escritos.push(Escrito(absoluto));
                if let Some(antes) = antes {
                    substituidos.push(raiz.join(&antes.caminho));
                }
                let sha =
                    (qual == Qual::ArquivoBruto).then(|| format!("{:x}", Sha256::digest(&b.bytes)));
                if let Some(sha) = &sha {
                    campos.insert("hashDoBruto".into(), Value::String(sha.clone()));
                }
                novas.insert(
                    qual.coluna(),
                    Some((
                        Guardada {
                            tipo: b.tipo,
                            nome: b.nome,
                            bytes: b.bytes.len() as i64,
                            caminho: relativo.to_string_lossy().into_owned(),
                        },
                        sha,
                    )),
                );
            }
        }
    }

    let galeria = texto(&campos, "galeriaId").unwrap_or_default();
    let criado_em = numero(&campos, "criadoEm");
    let ordem = numero(&campos, "ordem");
    let json = Value::Object(campos).to_string();
    if atual.is_none() {
        t.execute(
            "INSERT INTO importacao (id, galeria_id, criado_em, ordem, versao, item, atualizada_em)
             VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6)",
            params![g.id, galeria, criado_em, ordem, json, agora],
        )?;
    } else {
        t.execute(
            "UPDATE importacao SET galeria_id = ?2, criado_em = ?3, ordem = ?4, item = ?5,
             versao = ?6, atualizada_em = ?7 WHERE id = ?1",
            params![g.id, galeria, criado_em, ordem, json, versao, agora],
        )?;
    }
    for (coluna, nova) in novas {
        match nova {
            None => {
                let extra = if coluna == "bruto" {
                    ", bruto_sha256 = NULL"
                } else {
                    ""
                };
                t.execute(
                    &format!(
                        "UPDATE importacao SET {coluna} = NULL, {coluna}_tipo = NULL,
                         {coluna}_nome = NULL, {coluna}_bytes = NULL{extra} WHERE id = ?1"
                    ),
                    [&g.id],
                )?;
            }
            // Numa instrução só: o gatilho do bruto olha cada `UPDATE`, e o
            // SHA-256 gravado depois do caminho seria uma "troca".
            Some((guardada, Some(sha))) => {
                t.execute(
                    "UPDATE importacao SET bruto = ?2, bruto_tipo = ?3, bruto_nome = ?4,
                     bruto_bytes = ?5, bruto_sha256 = ?6 WHERE id = ?1",
                    params![
                        g.id,
                        guardada.caminho,
                        guardada.tipo,
                        guardada.nome,
                        guardada.bytes,
                        sha
                    ],
                )?;
            }
            Some((guardada, None)) => {
                t.execute(
                    &format!(
                        "UPDATE importacao SET {coluna} = ?2, {coluna}_tipo = ?3,
                         {coluna}_nome = ?4, {coluna}_bytes = ?5 WHERE id = ?1"
                    ),
                    params![
                        g.id,
                        guardada.caminho,
                        guardada.tipo,
                        guardada.nome,
                        guardada.bytes
                    ],
                )?;
            }
        }
    }
    let item = ler_na(t, &g.id)?.expect("o item acabou de ser gravado");
    Ok(Gravado::Pronto(Box::new(item)))
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    fn catalogo() -> (tempfile::TempDir, Catalogo) {
        let raiz = tempfile::tempdir().unwrap();
        let catalogo = Catalogo::abrir(raiz.path()).unwrap();
        (raiz, catalogo)
    }

    fn bytes(conteudo: &[u8]) -> Parte {
        Parte::Nova(Bytes {
            tipo: "image/jpeg".into(),
            nome: Some("a.jpg".into()),
            bytes: conteudo.to_vec(),
        })
    }

    fn novo(id: &str, galeria: &str) -> Gravacao {
        let campos = json!({"galeriaId": galeria, "criadoEm": 10, "ordem": 0, "nota": null, "estado": "aguardando"});
        let mut partes = BTreeMap::new();
        partes.insert("arquivo".to_string(), bytes(b"ARQ"));
        partes.insert("previa".to_string(), bytes(b"PRE"));
        Gravacao {
            id: id.into(),
            substituir: true,
            campos: campos.as_object().unwrap().clone(),
            partes,
            ..Default::default()
        }
    }

    fn pronto(g: Gravado) -> Item {
        match g {
            Gravado::Pronto(i) => *i,
            outro => panic!("esperava pronto: {outro:?}"),
        }
    }

    fn ler(c: &Catalogo, id: &str, qual: Qual) -> Option<Vec<u8>> {
        c.importacao_arquivo(id, qual)
            .unwrap()
            .map(|(p, _)| std::fs::read(p).unwrap())
    }

    #[test]
    fn grava_le_e_lista_na_ordem() {
        let (_r, mut c) = catalogo();
        let mut segundo = novo("b", "g1");
        segundo.campos.insert("criadoEm".into(), json!(20));
        c.importacao_gravar(vec![segundo, novo("a", "g1"), novo("z", "g2")], "t")
            .unwrap();
        let ids: Vec<_> = c
            .importacao_itens(Some("g1"))
            .unwrap()
            .into_iter()
            .map(|i| i.campos["id"].clone())
            .collect();
        assert_eq!(ids, vec![json!("a"), json!("b")]);
        assert_eq!(c.importacao_itens(None).unwrap().len(), 3);
        let a = c.importacao_item("a").unwrap().unwrap();
        assert_eq!(a.versao, 1);
        assert_eq!(a.arquivo.as_ref().unwrap().bytes, 3);
        assert!(a.bruto.is_none());
        assert_eq!(ler(&c, "a", Qual::Previa).unwrap(), b"PRE");
    }

    #[test]
    fn juntar_mantem_os_bytes_e_tira_o_que_era_undefined() {
        let (_r, mut c) = catalogo();
        let mut g = novo("a", "g1");
        g.campos.insert("erro".into(), json!("falhou"));
        c.importacao_gravar(vec![g], "t").unwrap();
        let patch = Gravacao {
            id: "a".into(),
            campos: json!({"progresso": 50}).as_object().unwrap().clone(),
            remover: vec!["erro".into()],
            ..Default::default()
        };
        let item = pronto(c.importacao_gravar(vec![patch], "t").unwrap().remove(0));
        assert_eq!(item.campos["progresso"], json!(50));
        assert!(!item.campos.contains_key("erro"));
        assert_eq!(item.campos["estado"], json!("aguardando"));
        assert_eq!(item.versao, 2);
        assert_eq!(ler(&c, "a", Qual::Arquivo).unwrap(), b"ARQ");
    }

    #[test]
    fn versao_velha_e_conflito_e_nada_muda() {
        let (_r, mut c) = catalogo();
        c.importacao_gravar(vec![novo("a", "g1")], "t").unwrap();
        let patch = |versao| Gravacao {
            id: "a".into(),
            campos: json!({"nota": 5}).as_object().unwrap().clone(),
            versao: Some(versao),
            ..Default::default()
        };
        pronto(c.importacao_gravar(vec![patch(1)], "t").unwrap().remove(0));
        assert_eq!(
            c.importacao_gravar(vec![patch(1)], "t").unwrap(),
            vec![Gravado::Conflito { id: "a".into() }]
        );
        assert_eq!(c.importacao_item("a").unwrap().unwrap().versao, 2);
    }

    #[test]
    fn substituir_sem_mandar_a_parte_nao_apaga_os_bytes() {
        let (_r, mut c) = catalogo();
        c.importacao_gravar(vec![novo("a", "g1")], "t").unwrap();
        let de_volta = Gravacao {
            id: "a".into(),
            substituir: true,
            campos: json!({"galeriaId": "g1", "estado": "aguardando"})
                .as_object()
                .unwrap()
                .clone(),
            ..Default::default()
        };
        let item = pronto(c.importacao_gravar(vec![de_volta], "t").unwrap().remove(0));
        assert!(!item.campos.contains_key("nota"));
        assert_eq!(ler(&c, "a", Qual::Arquivo).unwrap(), b"ARQ");
    }

    #[test]
    fn versao_zero_so_cria() {
        let (_r, mut c) = catalogo();
        let so_novo = |g: Gravacao| Gravacao {
            versao: Some(0),
            ..g
        };
        pronto(
            c.importacao_gravar(vec![so_novo(novo("a", "g1"))], "t")
                .unwrap()
                .remove(0),
        );
        assert_eq!(
            c.importacao_gravar(vec![so_novo(novo("a", "g2"))], "t")
                .unwrap(),
            vec![Gravado::Conflito { id: "a".into() }]
        );
    }

    #[test]
    fn juntar_em_item_ausente_nao_cria() {
        let (_r, mut c) = catalogo();
        let patch = Gravacao {
            id: "x".into(),
            ..Default::default()
        };
        assert_eq!(
            c.importacao_gravar(vec![patch], "t").unwrap(),
            vec![Gravado::Ausente { id: "x".into() }]
        );
        assert!(c.importacao_item("x").unwrap().is_none());
    }

    #[test]
    fn g4_o_bruto_nasce_uma_vez_com_o_sha256_daqui() {
        let (raiz, mut c) = catalogo();
        c.importacao_gravar(vec![novo("a", "g1")], "t").unwrap();
        let revelar = |arq: &[u8], bruto: &[u8]| {
            let mut partes = BTreeMap::new();
            partes.insert("arquivo".to_string(), bytes(arq));
            partes.insert("arquivoBruto".to_string(), bytes(bruto));
            Gravacao {
                id: "a".into(),
                campos: json!({"hashDoBruto": "mentira"})
                    .as_object()
                    .unwrap()
                    .clone(),
                partes,
                ..Default::default()
            }
        };
        let item = pronto(
            c.importacao_gravar(vec![revelar(b"REV1", b"BRUTO")], "t")
                .unwrap()
                .remove(0),
        );
        let sha = format!("{:x}", Sha256::digest(b"BRUTO"));
        assert_eq!(item.campos["hashDoBruto"], json!(sha));

        // A segunda revelação troca o arquivo, e não o bruto.
        let item = pronto(
            c.importacao_gravar(vec![revelar(b"REV2", b"OUTRO")], "t")
                .unwrap()
                .remove(0),
        );
        assert_eq!(item.campos["hashDoBruto"], json!(sha));
        assert_eq!(ler(&c, "a", Qual::ArquivoBruto).unwrap(), b"BRUTO");
        assert_eq!(ler(&c, "a", Qual::Arquivo).unwrap(), b"REV2");

        // Nem apagando, nem substituindo o item inteiro.
        let mut apagar = Gravacao {
            id: "a".into(),
            substituir: true,
            ..novo("a", "g1")
        };
        apagar.partes.insert("arquivoBruto".into(), Parte::Apagar);
        c.importacao_gravar(vec![apagar], "t").unwrap();
        assert_eq!(ler(&c, "a", Qual::ArquivoBruto).unwrap(), b"BRUTO");

        // E o banco recusa por conta própria.
        let erro = c
            .banco()
            .execute("UPDATE importacao SET bruto = 'x' WHERE id = 'a'", [])
            .unwrap_err();
        assert!(erro.to_string().contains("C2"), "{erro}");

        // Os arquivos substituídos saíram do disco.
        let pasta = raiz.path().join("importacao/a");
        let restantes: Vec<_> = std::fs::read_dir(pasta)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(restantes.len(), 3, "{restantes:?}");
    }

    #[test]
    fn uma_falha_no_lote_nao_grava_nada() {
        let (raiz, mut c) = catalogo();
        let ruim = Gravacao {
            id: "../fora".into(),
            ..novo("x", "g1")
        };
        assert!(c
            .importacao_gravar(vec![novo("a", "g1"), ruim], "t")
            .is_err());
        assert!(c.importacao_item("a").unwrap().is_none());
        assert!(!raiz.path().join("importacao/a").exists());
    }

    #[test]
    fn trocar_e_apagar_a_galeria() {
        let (raiz, mut c) = catalogo();
        c.importacao_gravar(vec![novo("a", "rascunho:1"), novo("b", "rascunho:1")], "t")
            .unwrap();
        let movidos = c
            .importacao_trocar_galeria("rascunho:1", "g9", "t")
            .unwrap();
        assert_eq!(movidos.len(), 2);
        assert_eq!(movidos[0].campos["galeriaId"], json!("g9"));
        assert!(c.importacao_itens(Some("rascunho:1")).unwrap().is_empty());
        let ids = c.importacao_apagar_galeria("g9").unwrap();
        assert_eq!(ids.len(), 2);
        assert!(!raiz.path().join("importacao/a").exists());
        assert!(c.importacao_itens(None).unwrap().is_empty());
    }

    #[test]
    fn apagar_um_item_leva_os_bytes() {
        let (raiz, mut c) = catalogo();
        c.importacao_gravar(vec![novo("rascunho:x", "g1")], "t")
            .unwrap();
        assert!(raiz.path().join("importacao/rascunho_x").exists());
        assert!(c.importacao_apagar("rascunho:x").unwrap());
        assert!(!raiz.path().join("importacao/rascunho_x").exists());
        assert!(!c.importacao_apagar("rascunho:x").unwrap());
    }
}
