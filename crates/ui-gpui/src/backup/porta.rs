//! A ponte entre a tela do backup e a API.
//!
//! Mesma forma das outras portas da casa (`Gravador`, `Marcador`, `Acervo`,
//! `Exportador`): o cliente HTTP é `async` do tokio, o GPUI não roda futuros
//! dele, e a resposta volta por `Sender`.
//!
//! 🚨 **Os bytes do arquivo não passam pela API.** `assinar` devolve uma URL do
//! R2, e [`Acervo::enviar`] dá `PUT` **nela** — direto, com progresso. É a mesma
//! decisão do site, e pelo mesmo motivo: uma pasta de ensaio são centenas de
//! arquivos, e atravessá-los pela máquina do Fly é o gargalo que a URL assinada
//! existe para não ter.

use std::sync::mpsc::Sender;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use domain::services::pos_venda::Sessao;
use serde::Deserialize;

/// Uma linha da listagem: arquivo ou pasta. O espelho de `EntradaDto`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EntradaDoAcervo {
    /// Relativo à raiz do acervo — é o que se navega e o que se devolve.
    pub caminho: String,
    pub nome: String,
    pub pasta: bool,
    /// `None` na pasta: o S3 não conta o que há dentro sem varrer tudo.
    pub bytes: Option<u64>,
    pub modificado_em: Option<DateTime<Utc>>,
}

/// Onde gravar um arquivo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DestinoDeEnvio {
    pub caminho: String,
    pub url: String,
    /// Quando `true`, o `PUT` vai para a própria API e leva o token; quando
    /// `false`, é URL assinada do R2 e **não pode** levar cabeçalho nosso.
    pub com_autenticacao: bool,
}

/// Um arquivo pronto para baixar, com onde buscá-lo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ParaBaixar {
    /// Relativo à pasta pedida — é o caminho dentro do zip.
    pub caminho: String,
    pub bytes: Option<u64>,
    pub url: String,
    pub com_autenticacao: bool,
}

/// A árvore de uma pasta, pronta para compactar.
#[derive(Debug, Clone, Deserialize)]
pub struct Arvore {
    pub arquivos: Vec<ParaBaixar>,
    pub nome_do_zip: String,
    pub maximo: usize,
}

#[derive(Debug, Deserialize)]
pub struct EnviosAssinados {
    pub envios: Vec<DestinoDeEnvio>,
    pub maximo_por_pedido: usize,
}

/// O que a tela precisa saber enquanto um arquivo sobe.
#[derive(Debug, Clone, PartialEq)]
pub enum Andamento {
    /// Quantos bytes já saíram **deste** arquivo.
    Subiu {
        id: usize,
        enviados: u64,
    },
    Terminou {
        id: usize,
    },
    /// ⚠️ **A falha de um arquivo não interrompe a fila** — a mesma regra da
    /// exportação em lote, e pelo mesmo motivo: numa pasta de 300 fotos, parar
    /// tudo por causa de uma obriga a recomeçar.
    Falhou {
        id: usize,
        erro: String,
    },
}

pub trait Acervo: Send + Sync + 'static {
    fn listar(
        &self,
        sessao: Sessao,
        caminho: String,
        canal: Sender<Result<Vec<EntradaDoAcervo>, String>>,
    );
    fn assinar(
        &self,
        sessao: Sessao,
        caminhos: Vec<String>,
        canal: Sender<Result<EnviosAssinados, String>>,
    );
    fn apagar(&self, sessao: Sessao, caminho: String, canal: Sender<Result<(), String>>);

    /// O link de leitura de **um** arquivo — a prévia e o download avulso.
    fn link(&self, sessao: Sessao, caminho: String, canal: Sender<Result<ParaBaixar, String>>);

    /// A árvore de uma pasta, com um link por arquivo.
    ///
    /// 🚨 **O zip é montado aqui, no app** — o backend só diz quais arquivos
    /// existem e assina o acesso. Os bytes vêm direto do R2, e a máquina do Fly
    /// não lê nem paga a saída de uma pasta de 2 GB.
    fn arvore(&self, sessao: Sessao, caminho: String, canal: Sender<Result<Arvore, String>>);

    /// Busca os bytes de um arquivo — do R2, ou da API na pilha local.
    fn baixar(
        &self,
        sessao: Sessao,
        arquivo: ParaBaixar,
        canal: Sender<Result<(String, Vec<u8>), String>>,
    );
    /// Sobe **um** arquivo, relatando o quanto já foi pelo canal.
    fn enviar(
        &self,
        sessao: Sessao,
        id: usize,
        destino: DestinoDeEnvio,
        corpo: Vec<u8>,
        tipo: String,
        canal: Sender<Andamento>,
    );
}

// ── A implementação HTTP ────────────────────────────────────────────────

use infrastructure::pos_venda::http::CorpoCru;
use infrastructure::pos_venda::PosVendaApiHttp;

/// O acervo pela API, com o token que o cliente já sabe renovar.
pub struct AcervoHttp {
    api: Arc<PosVendaApiHttp>,
    tokio: tokio::runtime::Handle,
}

impl AcervoHttp {
    pub fn novo(api: Arc<PosVendaApiHttp>, tokio: tokio::runtime::Handle) -> Self {
        Self { api, tokio }
    }
}

/// O corpo de uma resposta que não deu certo, curto o bastante para caber num
/// aviso.
///
/// O erro do R2 é um XML de várias linhas; o da API é o envelope JSON. Nenhum
/// dos dois cabe numa linha de tela, e o começo é onde está o que importa
/// (`SignatureDoesNotMatch`, `AccessDenied`).
fn resumo(texto: &str) -> String {
    let limpo = texto.trim().replace(['\n', '\r'], " ");
    if limpo.chars().count() <= 160 {
        return limpo;
    }
    limpo.chars().take(157).collect::<String>() + "…"
}

/// O caminho vira query, com espaço e acento codificados.
fn em_query(caminho: &str) -> String {
    // Percent-encoding à mão: o único caractere que precisa virar `%XX` aqui é
    // o que o servidor leria como separador. Uma dependência a mais para isto
    // seria desproporcional.
    let mut fora = String::with_capacity(caminho.len());
    for byte in caminho.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                fora.push(*byte as char);
            }
            outro => fora.push_str(&format!("%{outro:02X}")),
        }
    }
    fora
}

/// Uma resposta crua vira o que a tela espera, ou uma frase.
fn ler<T: for<'de> Deserialize<'de>>(
    resposta: Result<infrastructure::pos_venda::http::RespostaCrua, domain::DomainError>,
) -> Result<T, String> {
    let resposta = resposta.map_err(|e| e.to_string())?;
    let texto = String::from_utf8_lossy(&resposta.bytes);
    if !(200..300).contains(&resposta.status) {
        return Err(format!(
            "a API respondeu {}: {}",
            resposta.status,
            resumo(&texto)
        ));
    }
    serde_json::from_str(&texto).map_err(|e| format!("resposta ilegível: {e}"))
}

impl Acervo for AcervoHttp {
    fn listar(
        &self,
        sessao: Sessao,
        caminho: String,
        canal: Sender<Result<Vec<EntradaDoAcervo>, String>>,
    ) {
        let api = self.api.clone();
        self.tokio.spawn(async move {
            let rota = format!("/arquivos?caminho={}", em_query(&caminho));
            let _ = canal.send(ler(api.chamar(Some(&sessao), "GET", &rota, None).await));
        });
    }

    fn assinar(
        &self,
        sessao: Sessao,
        caminhos: Vec<String>,
        canal: Sender<Result<EnviosAssinados, String>>,
    ) {
        let api = self.api.clone();
        self.tokio.spawn(async move {
            let corpo = serde_json::json!({
                "arquivos": caminhos.iter().map(|c| serde_json::json!({ "caminho": c })).collect::<Vec<_>>(),
            });
            let pedido = CorpoCru {
                tipo: "application/json".into(),
                bytes: corpo.to_string().into_bytes(),
            };
            let _ = canal.send(ler(
                api.chamar(Some(&sessao), "POST", "/arquivos/envios", Some(pedido))
                    .await,
            ));
        });
    }

    fn apagar(&self, sessao: Sessao, caminho: String, canal: Sender<Result<(), String>>) {
        let api = self.api.clone();
        self.tokio.spawn(async move {
            let rota = format!("/arquivos?caminho={}", em_query(&caminho));
            let resposta = api.chamar(Some(&sessao), "DELETE", &rota, None).await;
            let _ = canal.send(match resposta {
                Ok(crua) if (200..300).contains(&crua.status) => Ok(()),
                Ok(crua) => Err(format!(
                    "a API respondeu {}: {}",
                    crua.status,
                    resumo(&String::from_utf8_lossy(&crua.bytes))
                )),
                Err(erro) => Err(erro.to_string()),
            });
        });
    }

    fn link(&self, sessao: Sessao, caminho: String, canal: Sender<Result<ParaBaixar, String>>) {
        let api = self.api.clone();
        self.tokio.spawn(async move {
            let rota = format!("/arquivos/link?caminho={}", em_query(&caminho));
            let _ = canal.send(ler(api.chamar(Some(&sessao), "GET", &rota, None).await));
        });
    }

    fn arvore(&self, sessao: Sessao, caminho: String, canal: Sender<Result<Arvore, String>>) {
        let api = self.api.clone();
        self.tokio.spawn(async move {
            let rota = format!("/arquivos/arvore?caminho={}", em_query(&caminho));
            let _ = canal.send(ler(api.chamar(Some(&sessao), "GET", &rota, None).await));
        });
    }

    fn baixar(
        &self,
        sessao: Sessao,
        arquivo: ParaBaixar,
        canal: Sender<Result<(String, Vec<u8>), String>>,
    ) {
        let api = self.api.clone();
        self.tokio.spawn(async move {
            let resultado = if arquivo.com_autenticacao {
                // 🖥️ Só a pilha local: o `GET` vai para a própria API, com o
                // token — pelo `chamar`, que é quem sabe renová-lo.
                let rota = format!("/arquivos/conteudo/{}", em_query(&arquivo.caminho));
                match api.chamar(Some(&sessao), "GET", &rota, None).await {
                    Ok(crua) if (200..300).contains(&crua.status) => Ok(crua.bytes),
                    Ok(crua) => Err(format!(
                        "a API respondeu {}: {}",
                        crua.status,
                        resumo(&String::from_utf8_lossy(&crua.bytes))
                    )),
                    Err(erro) => Err(erro.to_string()),
                }
            } else {
                api.buscar_url_assinada(&arquivo.url)
                    .await
                    .map_err(|e| e.to_string())
            };
            let _ = canal.send(resultado.map(|bytes| (arquivo.caminho, bytes)));
        });
    }

    fn enviar(
        &self,
        sessao: Sessao,
        id: usize,
        destino: DestinoDeEnvio,
        corpo: Vec<u8>,
        tipo: String,
        canal: Sender<Andamento>,
    ) {
        let api = self.api.clone();
        self.tokio.spawn(async move {
            let resultado = if destino.com_autenticacao {
                // 🖥️ **Só a pilha local.** Ali não há quem assine, e o `PUT` vai
                // para a própria API, com o token — pelo `chamar`, que é quem
                // sabe renová-lo. Sem progresso por byte: o corpo vai numa
                // tacada, e a barra desta peça salta de 0 a 100. É o ambiente
                // de desenvolvimento, e inventar um progresso interpolado
                // mostraria movimento onde não há informação.
                let bytes = corpo.len() as u64;
                let rota = format!("/arquivos/conteudo/{}", em_query(&destino.caminho));
                let crua = api
                    .chamar(
                        Some(&sessao),
                        "PUT",
                        &rota,
                        Some(CorpoCru { tipo, bytes: corpo }),
                    )
                    .await;
                match crua {
                    Ok(crua) if (200..300).contains(&crua.status) => {
                        let _ = canal.send(Andamento::Subiu {
                            id,
                            enviados: bytes,
                        });
                        Ok(())
                    }
                    Ok(crua) => Err(format!(
                        "a API respondeu {}: {}",
                        crua.status,
                        resumo(&String::from_utf8_lossy(&crua.bytes))
                    )),
                    Err(erro) => Err(erro.to_string()),
                }
            } else {
                let aviso = canal.clone();
                api.enviar_para_url_assinada(
                    &destino.url,
                    &tipo,
                    corpo,
                    Arc::new(move |enviados| {
                        let _ = aviso.send(Andamento::Subiu { id, enviados });
                    }),
                )
                .await
                .map_err(|e| e.to_string())
            };

            let _ = match resultado {
                Ok(()) => canal.send(Andamento::Terminou { id }),
                Err(erro) => canal.send(Andamento::Falhou {
                    id,
                    erro: resumo(&erro),
                }),
            };
        });
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn o_resumo_cabe_num_aviso() {
        assert_eq!(resumo("  erro\ncurto  "), "erro curto");
        let longo = "x".repeat(500);
        let cortado = resumo(&longo);
        assert_eq!(cortado.chars().count(), 158);
        assert!(cortado.ends_with('…'));
    }

    /// 🔑 A barra e o acento do caminho atravessam a query intactos — a pasta
    /// `acentuação/ensaio silva` existe, e é o nome que o cliente deu.
    #[test]
    fn a_query_preserva_a_pasta_e_codifica_o_resto() {
        assert_eq!(em_query("2026/ensaio"), "2026/ensaio");
        assert_eq!(em_query("ensaio silva"), "ensaio%20silva");
        assert_eq!(em_query("acentuação"), "acentua%C3%A7%C3%A3o");
        assert_eq!(em_query("a&b=c"), "a%26b%3Dc");
    }
}

/// A porta de mentira — a convenção da casa: é ela que deixa o teste afirmar
/// **o que foi pedido** e **em que ordem**, sem rede, sem R2 e sem token.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    /// 🔑 **`AcervoDeArquivos…`, e não `AcervoDeMentira`**: a Biblioteca já tem um
    /// `AcervoDeMentira` (o catálogo local), e os dois convivem no mesmo
    /// `portas()` dos testes. Dois nomes iguais ali obrigariam a apelidar um na
    /// importação, e o apelido é onde a troca silenciosa acontece.
    pub struct AcervoDeArquivosDeMentira {
        /// O que responder a cada `listar`, por caminho pedido.
        pub pastas: Mutex<std::collections::HashMap<String, Vec<EntradaDoAcervo>>>,
        pub listados: Mutex<Vec<String>>,
        /// Os caminhos de cada lote assinado, na ordem em que chegaram.
        pub assinados: Mutex<Vec<Vec<String>>>,
        pub apagados: Mutex<Vec<String>>,
        /// O conteúdo de cada arquivo, por caminho — o que `baixar` devolve.
        pub conteudo: Mutex<std::collections::HashMap<String, Vec<u8>>>,
        pub arvores: Mutex<std::collections::HashMap<String, Vec<ParaBaixar>>>,
        pub baixados: Mutex<Vec<String>>,
        /// O que foi enviado: `(caminho, tamanho)`.
        pub enviados: Mutex<Vec<(String, usize)>>,
        /// Quantos envios devem falhar, do começo — para a tela ser testada com
        /// falha no meio, que é o caso que ninguém reproduz à mão.
        pub falham: Mutex<usize>,
        /// A recusa que `assinar` devolve, quando há uma.
        pub recusa_a_assinatura: Mutex<Option<String>>,
    }

    impl Acervo for AcervoDeArquivosDeMentira {
        fn listar(
            &self,
            _sessao: Sessao,
            caminho: String,
            canal: Sender<Result<Vec<EntradaDoAcervo>, String>>,
        ) {
            self.listados.lock().unwrap().push(caminho.clone());
            let resposta = self
                .pastas
                .lock()
                .unwrap()
                .get(&caminho)
                .cloned()
                .unwrap_or_default();
            let _ = canal.send(Ok(resposta));
        }

        fn assinar(
            &self,
            _sessao: Sessao,
            caminhos: Vec<String>,
            canal: Sender<Result<EnviosAssinados, String>>,
        ) {
            self.assinados.lock().unwrap().push(caminhos.clone());
            if let Some(recusa) = self.recusa_a_assinatura.lock().unwrap().clone() {
                let _ = canal.send(Err(recusa));
                return;
            }
            let _ = canal.send(Ok(EnviosAssinados {
                envios: caminhos
                    .into_iter()
                    .map(|caminho| DestinoDeEnvio {
                        url: format!("https://r2.de-mentira/{caminho}"),
                        caminho,
                        com_autenticacao: false,
                    })
                    .collect(),
                maximo_por_pedido: 200,
            }));
        }

        fn apagar(&self, _sessao: Sessao, caminho: String, canal: Sender<Result<(), String>>) {
            self.apagados.lock().unwrap().push(caminho);
            let _ = canal.send(Ok(()));
        }

        fn link(
            &self,
            _sessao: Sessao,
            caminho: String,
            canal: Sender<Result<ParaBaixar, String>>,
        ) {
            let _ = canal.send(Ok(ParaBaixar {
                url: format!("https://r2.de-mentira/{caminho}"),
                caminho,
                bytes: None,
                com_autenticacao: false,
            }));
        }

        fn arvore(&self, _sessao: Sessao, caminho: String, canal: Sender<Result<Arvore, String>>) {
            let arquivos = self
                .arvores
                .lock()
                .unwrap()
                .get(&caminho)
                .cloned()
                .unwrap_or_default();
            let _ = canal.send(Ok(Arvore {
                nome_do_zip: format!(
                    "{}.zip",
                    caminho
                        .rsplit('/')
                        .next()
                        .filter(|n| !n.is_empty())
                        .unwrap_or("acervo")
                ),
                arquivos,
                maximo: 5_000,
            }));
        }

        fn baixar(
            &self,
            _sessao: Sessao,
            arquivo: ParaBaixar,
            canal: Sender<Result<(String, Vec<u8>), String>>,
        ) {
            self.baixados.lock().unwrap().push(arquivo.caminho.clone());
            let bytes = self
                .conteudo
                .lock()
                .unwrap()
                .get(&arquivo.caminho)
                .cloned()
                .unwrap_or_else(|| b"bytes de mentira".to_vec());
            let _ = canal.send(Ok((arquivo.caminho, bytes)));
        }

        fn enviar(
            &self,
            _sessao: Sessao,
            id: usize,
            destino: DestinoDeEnvio,
            corpo: Vec<u8>,
            _tipo: String,
            canal: Sender<Andamento>,
        ) {
            let tamanho = corpo.len();
            self.enviados
                .lock()
                .unwrap()
                .push((destino.caminho, tamanho));

            let mut falham = self.falham.lock().unwrap();
            if *falham > 0 {
                *falham -= 1;
                let _ = canal.send(Andamento::Falhou {
                    id,
                    erro: "falha combinada no teste".into(),
                });
                return;
            }
            // Meio caminho e o fim: é o que permite afirmar que a barra anda,
            // e não só que ela chega.
            let _ = canal.send(Andamento::Subiu {
                id,
                enviados: (tamanho / 2) as u64,
            });
            let _ = canal.send(Andamento::Subiu {
                id,
                enviados: tamanho as u64,
            });
            let _ = canal.send(Andamento::Terminou { id });
        }
    }
}
