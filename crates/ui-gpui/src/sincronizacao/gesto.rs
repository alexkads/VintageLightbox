//! Os gestos que a fila guarda — o que o balcão fez sem rede, um por linha da
//! tabela `fila_de_sincronizacao`.
//!
//! 🔑 **Cada gesto é a chamada da porta que ele repete**, com os mesmos
//! argumentos: o trabalhador da fila chama o `Publicador` de verdade como a
//! tela teria chamado. Nada aqui sabe HTTP.

use domain::services::pos_venda::{EstadoNoBalcao, MudancaDaFoto, MudancaDaGaleria, NovaGaleria};
use infrastructure::database::{GestoGuardado, GestoNovo};
use serde_json::{json, Value};

use crate::pos_venda::porta::FotoClassificada;

/// Um gesto do balcão que ainda não chegou ao site.
#[derive(Debug, Clone, PartialEq)]
pub enum Gesto {
    /// A sessão aberta sem rede — com o id escolhido aqui.
    CriarGaleria(NovaGaleria),
    /// Título, contato, estúdio ou associação da sessão.
    AtualizarGaleria {
        galeria_id: String,
        mudanca: MudancaDaGaleria,
    },
    /// Nota, levar (`B`), rejeição (`X`), faixa e negociação da foto que já
    /// está no site — o `PATCH /pos-venda/fotos/{id}`.
    MudarFoto {
        foto_id: String,
        mudanca: MudancaDaFoto,
    },
    /// A foto classificada que ainda não subiu (o passo 3).
    SubirClassificada {
        galeria_id: String,
        foto: FotoClassificada,
    },
    /// O "Salvar na galeria" de uma foto do site: a revelação em JSON, como
    /// o site a grava (ajustes e corte juntos).
    SalvarRevelacao {
        foto_no_site: String,
        parametros: Value,
    },
    /// O "Avisar cliente" — o e-mail de fotos prontas.
    Avisar { galeria_id: String },
    /// Um pedido JSON que grava — o caixa (abrir, vender, sangria e
    /// suprimento). O corpo já leva a chave de idempotência e a hora do balcão.
    PedidoJson {
        metodo: String,
        caminho: String,
        corpo: Value,
    },
}

impl Gesto {
    fn tipo(&self) -> &'static str {
        match self {
            Self::CriarGaleria(_) => "criar_galeria",
            Self::AtualizarGaleria { .. } => "atualizar_galeria",
            Self::MudarFoto { .. } => "mudar_foto",
            Self::SubirClassificada { .. } => "subir_classificada",
            Self::SalvarRevelacao { .. } => "salvar_revelacao",
            Self::Avisar { .. } => "avisar",
            Self::PedidoJson { .. } => "pedido_json",
        }
    }

    /// O que o gesto mexe — a foto ou a sessão.
    pub fn alvo(&self) -> &str {
        match self {
            Self::CriarGaleria(nova) => nova.id.as_deref().unwrap_or_default(),
            Self::AtualizarGaleria { galeria_id, .. } | Self::Avisar { galeria_id } => galeria_id,
            Self::MudarFoto { foto_id, .. } => foto_id,
            Self::SubirClassificada { foto, .. } => &foto.foto_id,
            Self::SalvarRevelacao { foto_no_site, .. } => foto_no_site,
            Self::PedidoJson { caminho, .. } => caminho,
        }
    }

    /// A sessão do gesto, quando ele sabe qual é.
    pub fn galeria_id(&self) -> Option<&str> {
        match self {
            Self::CriarGaleria(nova) => nova.id.as_deref(),
            Self::AtualizarGaleria { galeria_id, .. }
            | Self::Avisar { galeria_id }
            | Self::SubirClassificada { galeria_id, .. } => Some(galeria_id),
            Self::MudarFoto { .. } | Self::SalvarRevelacao { .. } => None,
            Self::PedidoJson { corpo, .. } => corpo.get("galeria_id").and_then(Value::as_str),
        }
    }

    fn corpo(&self) -> Value {
        match self {
            Self::CriarGaleria(nova) => nova.corpo(),
            Self::AtualizarGaleria { mudanca, .. } => mudanca.corpo(),
            Self::MudarFoto { mudanca, .. } => mudanca.corpo(),
            Self::SubirClassificada { foto, .. } => json!({
                "ordem": foto.ordem,
                "estado": foto.estado.map(EstadoNoBalcao::como_texto),
                "nota": foto.nota,
                "produto_id": foto.produto_id,
            }),
            Self::SalvarRevelacao { parametros, .. } => parametros.clone(),
            Self::Avisar { .. } => json!({}),
            Self::PedidoJson { metodo, corpo, .. } => json!({ "metodo": metodo, "corpo": corpo }),
        }
    }

    /// O gesto como a tabela o guarda, com a hora em que aconteceu.
    pub fn para_guardar(&self, chave: &str, ocorrido_em: &str) -> GestoNovo {
        GestoNovo {
            chave: chave.to_string(),
            tipo: self.tipo().to_string(),
            alvo: self.alvo().to_string(),
            galeria_id: self.galeria_id().map(str::to_string),
            corpo: self.corpo(),
            ocorrido_em: ocorrido_em.to_string(),
        }
    }

    /// O contrário de [`Gesto::para_guardar`]. `None` é um tipo que esta versão
    /// não conhece (gravado por uma versão mais nova) — ele fica na fila.
    pub fn do_guardado(guardado: &GestoGuardado) -> Option<Self> {
        let corpo = &guardado.corpo;
        let galeria = || guardado.galeria_id.clone().unwrap_or_default();
        Some(match guardado.tipo.as_str() {
            "criar_galeria" => Self::CriarGaleria(NovaGaleria::do_corpo(corpo)?),
            "atualizar_galeria" => Self::AtualizarGaleria {
                galeria_id: galeria(),
                mudanca: MudancaDaGaleria::do_corpo(corpo),
            },
            "mudar_foto" => Self::MudarFoto {
                foto_id: guardado.alvo.clone(),
                mudanca: MudancaDaFoto::do_corpo(corpo),
            },
            "subir_classificada" => Self::SubirClassificada {
                galeria_id: galeria(),
                foto: FotoClassificada {
                    foto_id: guardado.alvo.clone(),
                    ordem: corpo.get("ordem").and_then(Value::as_u64).unwrap_or(0) as u32,
                    estado: corpo
                        .get("estado")
                        .and_then(Value::as_str)
                        .and_then(EstadoNoBalcao::do_texto),
                    nota: corpo
                        .get("nota")
                        .and_then(Value::as_u64)
                        .and_then(|n| u8::try_from(n).ok()),
                    produto_id: corpo
                        .get("produto_id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                },
            },
            "salvar_revelacao" => Self::SalvarRevelacao {
                foto_no_site: guardado.alvo.clone(),
                parametros: corpo.clone(),
            },
            "avisar" => Self::Avisar {
                galeria_id: galeria(),
            },
            "pedido_json" => Self::PedidoJson {
                metodo: corpo.get("metodo")?.as_str()?.to_string(),
                caminho: guardado.alvo.clone(),
                corpo: corpo.get("corpo").cloned().unwrap_or(Value::Null),
            },
            _ => return None,
        })
    }

    /// A frase da linha na tela "Sincronização".
    pub fn descricao(&self) -> String {
        match self {
            Self::CriarGaleria(nova) => format!("Criar a sessão \"{}\"", nova.titulo),
            Self::AtualizarGaleria { .. } => "Editar os dados da sessão".to_string(),
            Self::MudarFoto { mudanca, .. } => {
                let mut partes = Vec::new();
                if let Some(nota) = mudanca.nota {
                    partes.push(match nota {
                        Some(n) => format!("nota {n}"),
                        None => "tirar a nota".to_string(),
                    });
                }
                if let Some(estado) = mudanca.estado {
                    partes.push(match estado {
                        EstadoNoBalcao::LevadaNoBalcao => "levar".to_string(),
                        EstadoNoBalcao::Disponivel => "deixar à venda".to_string(),
                    });
                }
                if let Some(rejeitada) = mudanca.rejeitada {
                    partes.push(
                        if rejeitada {
                            "rejeitar"
                        } else {
                            "desfazer a rejeição"
                        }
                        .into(),
                    );
                }
                if mudanca.preco_negociado.is_some() || mudanca.observacao_da_negociacao.is_some() {
                    partes.push("negociação".to_string());
                }
                if mudanca.produto_id.is_some() || mudanca.preco_de_venda.is_some() {
                    partes.push("faixa ou preço".to_string());
                }
                if partes.is_empty() {
                    "Mudar a foto".to_string()
                } else {
                    format!("Foto: {}", partes.join(", "))
                }
            }
            Self::SubirClassificada { .. } => "Subir a foto classificada".to_string(),
            Self::SalvarRevelacao { .. } => "Salvar a revelação na galeria".to_string(),
            Self::Avisar { .. } => "Avisar o cliente por e-mail".to_string(),
            Self::PedidoJson { caminho, corpo, .. } => super::caixa::descrever(caminho, corpo),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use infrastructure::database::EstadoDoGesto;

    fn guardado(novo: GestoNovo) -> GestoGuardado {
        GestoGuardado {
            id: 1,
            chave: novo.chave,
            tipo: novo.tipo,
            alvo: novo.alvo,
            galeria_id: novo.galeria_id,
            corpo: novo.corpo,
            ocorrido_em: novo.ocorrido_em,
            tentativas: 0,
            estado: EstadoDoGesto::Pendente,
            erro: None,
        }
    }

    /// Todo gesto volta da tabela igual ao que entrou.
    #[test]
    fn cada_gesto_vai_e_volta_pela_tabela() {
        let gestos = vec![
            Gesto::CriarGaleria(NovaGaleria {
                titulo: "Ana — 20/10".into(),
                email: None,
                whatsapp: Some("5554999990000".into()),
                produto_id: "p1".into(),
                estudio_id: Some("s1".into()),
                id: Some("8d7a3c1e-0000-4000-8000-000000000001".into()),
            }),
            Gesto::AtualizarGaleria {
                galeria_id: "g1".into(),
                mudanca: MudancaDaGaleria {
                    email: Some(Some("ana@x.com".into())),
                    ..Default::default()
                },
            },
            Gesto::MudarFoto {
                foto_id: "f1".into(),
                mudanca: MudancaDaFoto {
                    nota: Some(Some(5)),
                    estado: Some(EstadoNoBalcao::LevadaNoBalcao),
                    preco_negociado: Some(None),
                    ..Default::default()
                },
            },
            Gesto::SubirClassificada {
                galeria_id: "g1".into(),
                foto: FotoClassificada {
                    foto_id: "local-1".into(),
                    ordem: 7,
                    estado: Some(EstadoNoBalcao::Disponivel),
                    nota: Some(4),
                    produto_id: None,
                },
            },
            Gesto::SalvarRevelacao {
                foto_no_site: "f2".into(),
                parametros: json!({ "exposure": 0.5 }),
            },
            Gesto::Avisar {
                galeria_id: "g1".into(),
            },
            Gesto::PedidoJson {
                metodo: "POST".into(),
                caminho: "/pos-venda/caixa/vendas".into(),
                corpo: json!({ "galeria_id": "g1", "chave": "k", "itens": [] }),
            },
        ];
        for gesto in gestos {
            let volta = Gesto::do_guardado(&guardado(
                gesto.para_guardar("c", "2026-10-20T14:00:00-03:00"),
            ));
            assert_eq!(volta.as_ref(), Some(&gesto), "{gesto:?}");
        }
    }

    #[test]
    fn o_tipo_desconhecido_fica_na_fila() {
        let mut g = guardado(
            Gesto::Avisar {
                galeria_id: "g".into(),
            }
            .para_guardar("c", "x"),
        );
        g.tipo = "gesto_do_futuro".into();
        assert_eq!(Gesto::do_guardado(&g), None);
    }

    #[test]
    fn a_descricao_diz_o_que_o_operador_fez() {
        let gesto = Gesto::MudarFoto {
            foto_id: "f".into(),
            mudanca: MudancaDaFoto {
                nota: Some(Some(3)),
                estado: Some(EstadoNoBalcao::LevadaNoBalcao),
                ..Default::default()
            },
        };
        assert_eq!(gesto.descricao(), "Foto: nota 3, levar");
    }
}
