//! O depósito em disco do que ainda não chegou ao servidor: panics, erros e
//! reações ao aviso de versão.
//!
//! # Por que disco, e não memória
//!
//! 🚨 **O panic é justamente o momento em que não dá para confiar em nada** —
//! nem na rede, nem na thread do tokio, nem em o processo durar mais um
//! instante. O relato é escrito aqui, na hora, síncrono, e sobe depois: na
//! próxima abertura do fluxo, ou na próxima abertura do app. Um balcão sem
//! internet guarda tudo e entrega quando voltar.
//!
//! Um arquivo por relato, gravado em `.tmp` e renomeado: o processo que morre
//! no meio da escrita deixa um `.tmp`, que é ignorado, e nunca um JSON pela
//! metade.
//!
//! ⚠️ **Com teto.** Um erro que se repete a cada quadro encheria o disco do
//! balcão: acima de [`TETO`] arquivos, os mais antigos saem.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Quantos relatos esperam no disco, no máximo.
pub const TETO: usize = 300;

/// Um relato à espera do servidor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "especie", rename_all = "lowercase")]
pub enum Registro {
    /// Vai para `POST /app-desktop/ocorrencias`, com os campos da
    /// `domain::app_desktop::Ocorrencia` do backend.
    Ocorrencia(Ocorrencia),
    /// Vai para `POST /app-desktop/reacoes`.
    Reacao(Reacao),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ocorrencia {
    pub id: String,
    pub maquina_id: String,
    /// `panico` ou `erro`.
    pub tipo: String,
    pub versao: String,
    pub sistema: String,
    pub origem: String,
    pub mensagem: String,
    pub rastro: Option<String>,
    pub contexto: serde_json::Value,
    /// RFC 3339.
    pub ocorrida_em: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reacao {
    pub maquina: String,
    pub versao: String,
    pub reacao: String,
    pub detalhe: Option<String>,
}

/// A pasta do depósito, dentro da casa do app.
pub fn pasta_em(casa: &Path) -> PathBuf {
    casa.join("relatos")
}

/// Grava um relato. Nunca falha para quem chamou: relatar é acessório, e o
/// gancho do panic não tem a quem devolver erro.
pub fn guardar(pasta: &Path, registro: &Registro) {
    let Ok(bytes) = serde_json::to_vec(registro) else {
        return;
    };
    if std::fs::create_dir_all(pasta).is_err() {
        return;
    }
    // O nome ordena pela hora: é o que deixa podar os mais antigos sem ler.
    let nome = format!(
        "{:020}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        uuid::Uuid::new_v4().simple()
    );
    let temporario = pasta.join(format!("{nome}.tmp"));
    let definitivo = pasta.join(format!("{nome}.json"));
    if std::fs::write(&temporario, bytes).is_ok() {
        let _ = std::fs::rename(&temporario, &definitivo);
    }
    podar(pasta);
}

fn arquivos(pasta: &Path) -> Vec<PathBuf> {
    let mut lista: Vec<PathBuf> = std::fs::read_dir(pasta)
        .map(|l| {
            l.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect()
        })
        .unwrap_or_default();
    lista.sort();
    lista
}

fn podar(pasta: &Path) {
    let lista = arquivos(pasta);
    if lista.len() > TETO {
        for velho in &lista[..lista.len() - TETO] {
            let _ = std::fs::remove_file(velho);
        }
    }
}

/// Os relatos à espera, do mais antigo para o mais novo, com o arquivo de
/// cada um — para apagar só o que o servidor confirmou.
pub fn pendentes(pasta: &Path) -> Vec<(PathBuf, Registro)> {
    arquivos(pasta)
        .into_iter()
        .filter_map(|caminho| {
            let registro = std::fs::read(&caminho)
                .ok()
                .and_then(|b| serde_json::from_slice::<Registro>(&b).ok());
            match registro {
                Some(r) => Some((caminho, r)),
                None => {
                    // Ilegível não vai sair daqui nunca: apagado para não
                    // ocupar o teto.
                    let _ = std::fs::remove_file(&caminho);
                    None
                }
            }
        })
        .collect()
}

pub fn esquecer(caminhos: &[PathBuf]) {
    for caminho in caminhos {
        let _ = std::fs::remove_file(caminho);
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn erro(n: usize) -> Registro {
        Registro::Ocorrencia(Ocorrencia {
            id: format!("id-{n}"),
            maquina_id: "m".into(),
            tipo: "erro".into(),
            versao: "0.1.19".into(),
            sistema: "linux".into(),
            origem: "teste".into(),
            mensagem: format!("erro {n}"),
            rastro: None,
            contexto: serde_json::json!({}),
            ocorrida_em: "2026-09-26T12:00:00Z".into(),
        })
    }

    #[test]
    fn guarda_le_na_ordem_e_esquece() {
        let pasta = tempfile::TempDir::new().unwrap();
        guardar(pasta.path(), &erro(1));
        guardar(pasta.path(), &erro(2));
        let lidos = pendentes(pasta.path());
        assert_eq!(
            lidos.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>(),
            vec![erro(1), erro(2)]
        );
        esquecer(&[lidos[0].0.clone()]);
        assert_eq!(pendentes(pasta.path()).len(), 1);
    }

    #[test]
    fn o_teto_poda_os_mais_antigos() {
        let pasta = tempfile::TempDir::new().unwrap();
        for n in 0..TETO + 5 {
            guardar(pasta.path(), &erro(n));
        }
        let lidos = pendentes(pasta.path());
        assert_eq!(lidos.len(), TETO);
        assert_eq!(lidos[0].1, erro(5), "os cinco primeiros saíram");
    }

    #[test]
    fn o_tmp_e_o_ilegivel_nao_travam_o_resto() {
        let pasta = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(pasta.path()).unwrap();
        std::fs::write(pasta.path().join("0-morreu.tmp"), b"{meio").unwrap();
        std::fs::write(pasta.path().join("0-torto.json"), b"nao e json").unwrap();
        guardar(pasta.path(), &erro(1));
        assert_eq!(pendentes(pasta.path()).len(), 1);
        assert!(
            !pasta.path().join("0-torto.json").exists(),
            "o ilegível foi apagado"
        );
    }

    #[test]
    fn o_json_da_ocorrencia_e_o_que_o_backend_le() {
        let Registro::Ocorrencia(o) = erro(1) else {
            unreachable!()
        };
        let json = serde_json::to_value(&o).unwrap();
        for campo in [
            "id",
            "maquina_id",
            "tipo",
            "versao",
            "sistema",
            "origem",
            "mensagem",
            "rastro",
            "contexto",
            "ocorrida_em",
        ] {
            assert!(json.get(campo).is_some(), "falta {campo}");
        }
    }
}
