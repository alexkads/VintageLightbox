//! A fila de envios no catálogo (G6, G7): o que a página entregou para subir, e
//! em que pé está cada um.
//!
//! O arquivo de cada envio fica em `envios/<chave>/`, e a linha da fila guarda
//! o resto (o endereço do bilhete, os campos e a lista dos arquivos). Os dois
//! só saem **depois** de o servidor confirmar.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{Catalogo, ErroDoCatalogo};

pub const OPERACAO_ENVIO: &str = "envio";

/// A espera máxima entre duas tentativas.
const ESPERA_MAXIMA: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArquivoDoEnvio {
    pub campo: String,
    pub nome: String,
    pub tipo: String,
    /// Relativo à raiz do catálogo.
    pub arquivo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DadosDoEnvio {
    /// O endereço do bilhete, como a página o recebeu (`…/envio-publico/…`).
    pub url: String,
    pub campos: BTreeMap<String, String>,
    pub arquivos: Vec<ArquivoDoEnvio>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvioNaFila {
    pub id: i64,
    pub chave: String,
    pub dados: DadosDoEnvio,
    pub tentativas: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Contagem {
    pub pendentes: i64,
    pub recusados: i64,
}

/// O que aconteceu com uma tentativa.
/// O que [`Catalogo::guardar_envio`] fez com a entrega.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guardado {
    Novo,
    Substituido,
    Ocupado,
}

/// Um envio que o servidor recusou.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recusado {
    pub chave: String,
    pub motivo: String,
    pub quando: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Desfecho {
    /// O servidor aceitou (ou já tinha, 409). O envio sai da fila.
    Feito,
    /// Não adianta insistir (4xx). Fica registrado, com o motivo, à vista.
    Recusado(String),
    /// Rede ou servidor (5xx, 429). Volta à fila, esperando mais a cada vez.
    TentarDepois(String),
}

impl Catalogo {
    /// Guarda um envio que a página entregou.
    ///
    /// A chave é a do depósito da página (o id da foto ou do item), e a entrega
    /// segue o `put` do IndexedDB: a mais nova vale.
    ///
    /// - sem linha, ou a anterior já `feito`: nasce uma linha. A anterior fica
    ///   no diário com a chave `<chave>~<id>`;
    /// - a anterior `pendente` ou `recusado`: ela passa a levar os bytes novos,
    ///   e volta a esperar a vez;
    /// - a anterior `enviando`: [`Guardado::Ocupado`]. A página fica com o envio
    ///   e entrega de novo depois.
    pub fn guardar_envio(
        &self,
        chave: &str,
        url: &str,
        campos: BTreeMap<String, String>,
        arquivos: Vec<(String, String, String, Vec<u8>)>,
        agora: i64,
    ) -> Result<Guardado, ErroDoCatalogo> {
        if !chave_segura(chave) {
            return Err(ErroDoCatalogo::Entrada(format!(
                "chave de envio inválida: {chave}"
            )));
        }
        let anterior: Option<(i64, String)> = self
            .banco
            .query_row(
                "SELECT id, estado FROM fila WHERE chave = ?1",
                [chave],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .optional()?;
        let quando = carimbo(agora);
        let substituir = match &anterior {
            Some((_, estado)) if estado == "enviando" => return Ok(Guardado::Ocupado),
            Some((id, estado)) if estado == "feito" => {
                self.banco.execute(
                    "UPDATE fila SET chave = chave || '~' || id WHERE id = ?1",
                    [id],
                )?;
                None
            }
            Some((id, _)) => Some(*id),
            None => None,
        };

        let pasta = PathBuf::from("envios").join(chave);
        let absoluta = self.raiz.join(&pasta);
        let _ = std::fs::remove_dir_all(&absoluta);
        std::fs::create_dir_all(&absoluta)
            .map_err(|e| ErroDoCatalogo::Pasta(absoluta.clone(), e))?;
        let mut lista = Vec::with_capacity(arquivos.len());
        for (indice, (campo, nome, tipo, bytes)) in arquivos.into_iter().enumerate() {
            let relativo = pasta.join(format!("{indice}.bin"));
            std::fs::write(self.raiz.join(&relativo), &bytes)
                .map_err(|e| ErroDoCatalogo::Pasta(absoluta.clone(), e))?;
            lista.push(ArquivoDoEnvio {
                campo,
                nome,
                tipo,
                arquivo: relativo.to_string_lossy().into_owned(),
            });
        }
        let dados = DadosDoEnvio {
            url: url.to_string(),
            campos,
            arquivos: lista,
        };
        let json = serde_json::to_string(&dados).expect("dados do envio em JSON");
        let resultado = match substituir {
            Some(id) => self.banco.execute(
                "UPDATE fila SET dados = ?2, estado = 'pendente', motivo = NULL, tentativas = 0,
                 tentar_depois_de = 0, atualizada_em = ?3 WHERE id = ?1",
                params![id, json, quando],
            ),
            None => self.banco.execute(
                "INSERT INTO fila (chave, operacao, dados, criada_em, atualizada_em)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![chave, OPERACAO_ENVIO, json, quando],
            ),
        };
        if let Err(erro) = resultado {
            // Sem a linha, os arquivos não teriam dono.
            let _ = std::fs::remove_dir_all(&absoluta);
            return Err(erro.into());
        }
        Ok(if substituir.is_some() {
            Guardado::Substituido
        } else {
            Guardado::Novo
        })
    }

    /// A página desistiu de um envio (a edição foi descartada). Um envio que já
    /// está subindo segue; o que espera sai da fila, com os bytes.
    pub fn esquecer_envio(&self, chave: &str) -> Result<bool, ErroDoCatalogo> {
        if !chave_segura(chave) {
            return Ok(false);
        }
        let apagadas = self.banco.execute(
            "DELETE FROM fila WHERE chave = ?1 AND operacao = ?2
             AND estado IN ('pendente', 'recusado')",
            params![chave, OPERACAO_ENVIO],
        )?;
        if apagadas > 0 {
            let _ = std::fs::remove_dir_all(self.raiz.join("envios").join(chave));
        }
        Ok(apagadas > 0)
    }

    /// Os envios recusados, com o motivo, para o operador ver (G7).
    pub fn envios_recusados(&self) -> Result<Vec<Recusado>, ErroDoCatalogo> {
        let mut consulta = self.banco.prepare(
            "SELECT chave, motivo, atualizada_em FROM fila
             WHERE estado = 'recusado' AND operacao = ?1 ORDER BY id",
        )?;
        let linhas = consulta.query_map([OPERACAO_ENVIO], |l| {
            Ok(Recusado {
                chave: l.get(0)?,
                motivo: l.get(1)?,
                quando: l.get(2)?,
            })
        })?;
        Ok(linhas.collect::<Result<_, _>>()?)
    }

    /// O próximo envio que pode ser tentado agora, já marcado `enviando`.
    pub fn proximo_envio(&self, agora: i64) -> Result<Option<EnvioNaFila>, ErroDoCatalogo> {
        let linha = self
            .banco
            .query_row(
                "SELECT id, chave, dados, tentativas FROM fila
                 WHERE estado = 'pendente' AND operacao = ?1 AND tentar_depois_de <= ?2
                 ORDER BY id LIMIT 1",
                params![OPERACAO_ENVIO, agora],
                |l| {
                    Ok((
                        l.get::<_, i64>(0)?,
                        l.get::<_, String>(1)?,
                        l.get::<_, String>(2)?,
                        l.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, chave, dados, tentativas)) = linha else {
            return Ok(None);
        };
        self.banco.execute(
            "UPDATE fila SET estado = 'enviando', atualizada_em = ?2 WHERE id = ?1",
            params![id, carimbo(agora)],
        )?;
        let dados: DadosDoEnvio = serde_json::from_str(&dados)
            .map_err(|e| ErroDoCatalogo::Entrada(format!("envio {chave} ilegível: {e}")))?;
        Ok(Some(EnvioNaFila {
            id,
            chave,
            dados,
            tentativas,
        }))
    }

    /// Registra o desfecho de uma tentativa.
    pub fn concluir_envio(
        &self,
        envio: &EnvioNaFila,
        desfecho: &Desfecho,
        agora: i64,
    ) -> Result<(), ErroDoCatalogo> {
        let quando = carimbo(agora);
        match desfecho {
            Desfecho::Feito => {
                self.banco.execute(
                    "UPDATE fila SET estado = 'feito', motivo = NULL, atualizada_em = ?2 WHERE id = ?1",
                    params![envio.id, quando],
                )?;
                // Os bytes só saem depois de a linha dizer "feito".
                let _ = std::fs::remove_dir_all(self.raiz.join("envios").join(&envio.chave));
            }
            Desfecho::Recusado(motivo) => {
                self.banco.execute(
                    "UPDATE fila SET estado = 'recusado', motivo = ?2, tentativas = tentativas + 1,
                     atualizada_em = ?3 WHERE id = ?1",
                    params![envio.id, motivo, quando],
                )?;
            }
            Desfecho::TentarDepois(motivo) => {
                let espera = espera_depois_de(envio.tentativas + 1);
                self.banco.execute(
                    "UPDATE fila SET estado = 'pendente', motivo = ?2, tentativas = tentativas + 1,
                     tentar_depois_de = ?3, atualizada_em = ?4 WHERE id = ?1",
                    params![envio.id, motivo, agora + espera, quando],
                )?;
            }
        }
        Ok(())
    }

    /// Troca o endereço do bilhete de um envio (o anterior venceu).
    pub fn trocar_bilhete(&self, envio: &mut EnvioNaFila, url: &str) -> Result<(), ErroDoCatalogo> {
        envio.dados.url = url.to_string();
        let json = serde_json::to_string(&envio.dados).expect("dados do envio em JSON");
        self.banco.execute(
            "UPDATE fila SET dados = ?2 WHERE id = ?1",
            params![envio.id, json],
        )?;
        Ok(())
    }

    /// O que estava `enviando` quando o app caiu volta a `pendente`.
    pub fn retomar_envios(&self) -> Result<usize, ErroDoCatalogo> {
        Ok(self.banco.execute(
            "UPDATE fila SET estado = 'pendente', tentar_depois_de = 0 WHERE estado = 'enviando'",
            [],
        )?)
    }

    /// Chama de novo o que estava esperando: a rede voltou, ou o operador pediu.
    pub fn tentar_ja(&self) -> Result<usize, ErroDoCatalogo> {
        Ok(self.banco.execute(
            "UPDATE fila SET tentar_depois_de = 0 WHERE estado = 'pendente'",
            [],
        )?)
    }

    /// Quando algo chegou ao servidor pela última vez: pela fila, ou pela área
    /// temporária (a importação sobe pelo worker da página).
    pub fn ultimo_envio_concluido(&self) -> Result<Option<String>, ErroDoCatalogo> {
        let datas: Vec<String> = {
            let mut c = self.banco.prepare(
                "SELECT max(atualizada_em) FROM fila WHERE estado = 'feito'
                 UNION ALL
                 SELECT max(atualizada_em) FROM importacao
                 WHERE json_extract(item, '$.estado') = 'pronta'",
            )?;
            let datas = c
                .query_map([], |l| l.get::<_, Option<String>>(0))?
                .filter_map(|d| d.ok().flatten())
                .collect();
            datas
        };
        // Os dois carimbos têm formatos RFC 3339 um pouco diferentes: compara-se
        // o instante, e não o texto.
        Ok(datas
            .into_iter()
            .filter_map(|d| chrono::DateTime::parse_from_rfc3339(&d).ok())
            .max()
            .map(|d| d.to_rfc3339()))
    }

    /// O envio que falhou e espera a próxima tentativa: quando, e por quê.
    pub fn proxima_tentativa(&self) -> Result<Option<(i64, Option<String>)>, ErroDoCatalogo> {
        Ok(self
            .banco
            .query_row(
                "SELECT tentar_depois_de, motivo FROM fila
                 WHERE estado = 'pendente' AND tentativas > 0 AND operacao = ?1
                 ORDER BY tentar_depois_de LIMIT 1",
                [OPERACAO_ENVIO],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .optional()?)
    }

    pub fn contagem_da_fila(&self) -> Result<Contagem, ErroDoCatalogo> {
        Ok(self.banco.query_row(
            "SELECT
                 coalesce(sum(estado IN ('pendente', 'enviando')), 0),
                 coalesce(sum(estado = 'recusado'), 0)
             FROM fila WHERE operacao = ?1",
            [OPERACAO_ENVIO],
            |l| {
                Ok(Contagem {
                    pendentes: l.get(0)?,
                    recusados: l.get(1)?,
                })
            },
        )?)
    }

    pub fn ler_arquivo_do_envio(
        &self,
        arquivo: &ArquivoDoEnvio,
    ) -> Result<Vec<u8>, ErroDoCatalogo> {
        let caminho = self.raiz.join(&arquivo.arquivo);
        std::fs::read(&caminho).map_err(|e| ErroDoCatalogo::Pasta(caminho, e))
    }
}

/// A espera antes da tentativa `n` (1, 2, …): 5 s, 10 s, 20 s… até 5 min.
pub fn espera_depois_de(n: i64) -> i64 {
    let expoente = (n - 1).clamp(0, 10) as u32;
    (5_i64 * 2_i64.pow(expoente)).min(ESPERA_MAXIMA)
}

/// A chave vira nome de pasta: só letras, números, `-` e `_`.
fn chave_segura(chave: &str) -> bool {
    !chave.is_empty()
        && chave.len() <= 128
        && chave
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn carimbo(segundos: i64) -> String {
    chrono::DateTime::from_timestamp(segundos, 0)
        .unwrap_or_default()
        .to_rfc3339()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn catalogo() -> (tempfile::TempDir, Catalogo) {
        let raiz = tempfile::tempdir().unwrap();
        let catalogo = Catalogo::abrir(raiz.path()).unwrap();
        (raiz, catalogo)
    }

    fn guardar(catalogo: &Catalogo, chave: &str) -> Guardado {
        guardar_bytes(catalogo, chave, vec![1, 2, 3])
    }

    fn guardar_bytes(catalogo: &Catalogo, chave: &str, bytes: Vec<u8>) -> Guardado {
        let mut campos = BTreeMap::new();
        campos.insert("nota".to_string(), "5".to_string());
        catalogo
            .guardar_envio(
                chave,
                "vlb://localhost/envio-publico/api/v2/public/pos-venda/envio/x",
                campos,
                vec![("file".into(), "a.jpg".into(), "image/jpeg".into(), bytes)],
                1000,
            )
            .unwrap()
    }

    #[test]
    fn a_entrega_mais_nova_da_mesma_chave_vale() {
        let (_raiz, catalogo) = catalogo();
        assert_eq!(guardar(&catalogo, "k1"), Guardado::Novo);
        assert_eq!(
            guardar_bytes(&catalogo, "k1", vec![9]),
            Guardado::Substituido
        );
        assert_eq!(catalogo.contagem_da_fila().unwrap().pendentes, 1);
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        assert_eq!(
            catalogo
                .ler_arquivo_do_envio(&envio.dados.arquivos[0])
                .unwrap(),
            vec![9]
        );
    }

    #[test]
    fn a_mesma_chave_enviando_fica_com_a_pagina() {
        let (_raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        assert_eq!(guardar_bytes(&catalogo, "k1", vec![9]), Guardado::Ocupado);
        assert_eq!(
            catalogo
                .ler_arquivo_do_envio(&envio.dados.arquivos[0])
                .unwrap(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn a_mesma_chave_depois_de_feito_nasce_de_novo_e_o_diario_fica() {
        let (_raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        catalogo
            .concluir_envio(&envio, &Desfecho::Feito, 1001)
            .unwrap();
        assert_eq!(guardar_bytes(&catalogo, "k1", vec![9]), Guardado::Novo);
        let novo = catalogo.proximo_envio(1002).unwrap().unwrap();
        assert_eq!(
            catalogo
                .ler_arquivo_do_envio(&novo.dados.arquivos[0])
                .unwrap(),
            vec![9]
        );
        let linhas: i64 = catalogo
            .banco()
            .query_row("SELECT count(*) FROM fila", [], |l| l.get(0))
            .unwrap();
        assert_eq!(linhas, 2);
    }

    #[test]
    fn esquecer_tira_o_que_espera_e_nao_o_que_sobe() {
        let (raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        guardar(&catalogo, "k2");
        assert!(catalogo.esquecer_envio("k1").unwrap());
        assert!(!raiz.path().join("envios/k1").exists());
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        assert_eq!(envio.chave, "k2");
        assert!(!catalogo.esquecer_envio("k2").unwrap());
        assert!(!catalogo.esquecer_envio("../x").unwrap());
    }

    #[test]
    fn uma_chave_nao_vira_caminho_fora_da_pasta() {
        let (_raiz, catalogo) = catalogo();
        for chave in ["../x", "a/b", "", "a b"] {
            assert!(
                catalogo
                    .guardar_envio(chave, "u", BTreeMap::new(), vec![], 0)
                    .is_err(),
                "{chave}"
            );
        }
    }

    #[test]
    fn feito_sai_da_fila_e_apaga_os_bytes() {
        let (raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        assert_eq!(
            catalogo
                .ler_arquivo_do_envio(&envio.dados.arquivos[0])
                .unwrap(),
            vec![1, 2, 3]
        );
        assert!(
            catalogo.proximo_envio(1000).unwrap().is_none(),
            "já está enviando"
        );
        catalogo
            .concluir_envio(&envio, &Desfecho::Feito, 1001)
            .unwrap();
        assert!(!raiz.path().join("envios/k1").exists());
        assert_eq!(catalogo.contagem_da_fila().unwrap(), Contagem::default());
    }

    #[test]
    fn g7_recusado_fica_a_vista_com_o_motivo_e_os_bytes() {
        let (raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        catalogo
            .concluir_envio(
                &envio,
                &Desfecho::Recusado("410: foto apagada".into()),
                1001,
            )
            .unwrap();
        assert_eq!(
            catalogo.contagem_da_fila().unwrap(),
            Contagem {
                pendentes: 0,
                recusados: 1
            }
        );
        assert!(raiz.path().join("envios/k1/0.bin").exists());
        assert!(catalogo.proximo_envio(99_999).unwrap().is_none());
        assert_eq!(
            catalogo.envios_recusados().unwrap(),
            vec![Recusado {
                chave: "k1".into(),
                motivo: "410: foto apagada".into(),
                quando: carimbo(1001),
            }]
        );
    }

    #[test]
    fn falha_de_rede_espera_cada_vez_mais() {
        let (_raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        catalogo
            .concluir_envio(&envio, &Desfecho::TentarDepois("sem rede".into()), 1000)
            .unwrap();
        assert!(catalogo.proximo_envio(1004).unwrap().is_none());
        let de_novo = catalogo.proximo_envio(1005).unwrap().unwrap();
        assert_eq!(de_novo.tentativas, 1);
        catalogo
            .concluir_envio(&de_novo, &Desfecho::TentarDepois("sem rede".into()), 1005)
            .unwrap();
        assert!(catalogo.proximo_envio(1014).unwrap().is_none());
        assert!(catalogo.proximo_envio(1015).unwrap().is_some());
    }

    #[test]
    fn o_ultimo_envio_e_a_proxima_tentativa() {
        let (_raiz, catalogo) = catalogo();
        assert_eq!(catalogo.ultimo_envio_concluido().unwrap(), None);
        assert_eq!(catalogo.proxima_tentativa().unwrap(), None);
        guardar(&catalogo, "k1");
        guardar(&catalogo, "k2");
        let um = catalogo.proximo_envio(1000).unwrap().unwrap();
        catalogo
            .concluir_envio(&um, &Desfecho::Feito, 2000)
            .unwrap();
        let dois = catalogo.proximo_envio(2000).unwrap().unwrap();
        catalogo
            .concluir_envio(&dois, &Desfecho::TentarDepois("sem rede".into()), 2000)
            .unwrap();
        assert_eq!(
            catalogo.ultimo_envio_concluido().unwrap(),
            Some(carimbo(2000))
        );
        assert_eq!(
            catalogo.proxima_tentativa().unwrap(),
            Some((2005, Some("sem rede".into())))
        );
    }

    #[test]
    fn a_espera_tem_teto() {
        assert_eq!(espera_depois_de(1), 5);
        assert_eq!(espera_depois_de(2), 10);
        assert_eq!(espera_depois_de(7), 300);
        assert_eq!(espera_depois_de(50), 300);
    }

    #[test]
    fn o_que_estava_enviando_quando_o_app_caiu_volta() {
        let (_raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        catalogo.proximo_envio(1000).unwrap().unwrap();
        assert!(catalogo.proximo_envio(1000).unwrap().is_none());
        assert_eq!(catalogo.retomar_envios().unwrap(), 1);
        assert!(catalogo.proximo_envio(1000).unwrap().is_some());
    }

    #[test]
    fn tentar_ja_ignora_a_espera() {
        let (_raiz, catalogo) = catalogo();
        guardar(&catalogo, "k1");
        let envio = catalogo.proximo_envio(1000).unwrap().unwrap();
        catalogo
            .concluir_envio(&envio, &Desfecho::TentarDepois("sem rede".into()), 1000)
            .unwrap();
        assert!(catalogo.proximo_envio(1001).unwrap().is_none());
        catalogo.tentar_ja().unwrap();
        assert!(catalogo.proximo_envio(1001).unwrap().is_some());
    }

    #[test]
    fn a_ordem_e_a_de_chegada() {
        let (_raiz, catalogo) = catalogo();
        guardar(&catalogo, "b");
        guardar(&catalogo, "a");
        assert_eq!(catalogo.proximo_envio(1000).unwrap().unwrap().chave, "b");
        assert_eq!(catalogo.proximo_envio(1000).unwrap().unwrap().chave, "a");
    }
}
