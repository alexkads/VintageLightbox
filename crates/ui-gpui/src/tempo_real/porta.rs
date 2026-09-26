//! A porta do tempo real: fluxos SSE da API abertos enquanto a conta está
//! dentro, reconectando sozinhos, e o que chega vira [`Sinal`] num canal.
//!
//! É o `components/tempo-real/canal.tsx` do site, com duas diferenças que vêm
//! de o app não ser uma aba:
//!
//! - **Fica aberto com a janela escondida.** No site a conexão só vive com a aba
//!   visível; aqui o operador está na triagem, ou com o app na bandeja, e é
//!   justamente aí que ele precisa do aviso.
//! - **Fala direto com a API**, com o token do app. O site passa pelo proxy do
//!   Next (`/api/chatbot/eventos`), que lê o cookie — e o app não tem cookie.
//!
//! A política de reconexão é a do site (`politica.ts`): espera de
//! `min(1 s × 2ⁿ, 30 s)` mais até 30% de sorteio, e `401`/`403` é permanente —
//! reconectar em laço não cura permissão.

use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use domain::services::pos_venda::Sessao;
use domain::DomainError;
use infrastructure::pos_venda::PosVendaApiHttp;

use super::sse::LeitorSse;

/// O `keep-alive` do servidor sai a cada 15 s; três sem chegar é conexão morta.
pub const SILENCIO_MAXIMO: Duration = Duration::from_secs(45);
const ESPERA_INICIAL: Duration = Duration::from_secs(1);
const ESPERA_MAXIMA: Duration = Duration::from_secs(30);

/// Como está a conexão de uma fonte — os quatro estados do indicador do site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoDaConexao {
    Conectando,
    Conectado,
    Reconectando,
    /// `401`/`403`: não tenta mais. A tela cai na releitura periódica.
    Recusado,
}

impl EstadoDaConexao {
    /// O estado de várias fontes juntas é o da pior — como a fusão do site, que
    /// só diz "pronto" quando todas disseram.
    pub fn de_todas(estados: impl IntoIterator<Item = EstadoDaConexao>) -> EstadoDaConexao {
        use EstadoDaConexao::*;
        let peso = |e: EstadoDaConexao| match e {
            Conectado => 0,
            Conectando => 1,
            Reconectando => 2,
            Recusado => 3,
        };
        estados
            .into_iter()
            .max_by_key(|e| peso(*e))
            .unwrap_or(Conectando)
    }

    /// O texto do `indicador.tsx`.
    pub fn rotulo(self) -> &'static str {
        match self {
            EstadoDaConexao::Conectando => "Conectando",
            EstadoDaConexao::Conectado => "Tempo real",
            EstadoDaConexao::Reconectando => "Reconectando",
            EstadoDaConexao::Recusado => "Atualizando a cada 15s",
        }
    }
}

/// O que a porta diz à tela. `fonte` é o rótulo que a tela deu ao fluxo.
#[derive(Debug, Clone, PartialEq)]
pub enum Sinal {
    Conexao {
        fonte: &'static str,
        estado: EstadoDaConexao,
    },
    /// O fluxo abriu (de novo): o que aconteceu no intervalo se perdeu, e a
    /// tela relê.
    Pronto { fonte: &'static str },
    /// O servidor avisou que o painel ficou para trás — relê.
    Sincronizar { fonte: &'static str },
    Evento {
        fonte: &'static str,
        dados: serde_json::Value,
    },
}

/// Enquanto viva, os fluxos ficam abertos. Largá-la fecha todos — é o "sair
/// da conta".
pub struct Guarda {
    _parar: Box<dyn Send>,
}

impl Guarda {
    pub fn nova(parar: impl Send + 'static) -> Self {
        Self {
            _parar: Box::new(parar),
        }
    }
}

pub trait Escuta: Send + Sync + 'static {
    /// Abre um fluxo por `(fonte, caminho)` e manda os sinais pelo canal.
    /// Devolve na hora.
    fn escutar(
        &self,
        sessao: Sessao,
        fontes: Vec<(&'static str, &'static str)>,
        canal: Sender<Sinal>,
    ) -> Guarda;
}

/// A espera antes da tentativa `n` (0 = a primeira reconexão), sem o sorteio.
pub fn espera_antes_da_tentativa(n: u32) -> Duration {
    let fator = 2u32.saturating_pow(n.min(16));
    (ESPERA_INICIAL * fator).min(ESPERA_MAXIMA)
}

/// Até 30% a mais, para cinco painéis que caíram juntos não voltarem juntos.
fn com_sorteio(espera: Duration) -> Duration {
    let semente = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    espera.mul_f64(1.0 + (semente % 300) as f64 / 1000.0)
}

// ── A implementação HTTP ────────────────────────────────────────────────

/// Pelos fluxos da API, com o mesmo cliente (e o mesmo token) do resto do app.
pub struct EscutaHttp {
    api: Arc<PosVendaApiHttp>,
    tokio: tokio::runtime::Handle,
}

impl EscutaHttp {
    pub fn nova(api: Arc<PosVendaApiHttp>, tokio: tokio::runtime::Handle) -> Self {
        Self { api, tokio }
    }
}

/// Aborta as tarefas quando a guarda cai.
struct Tarefas(Vec<tokio::task::JoinHandle<()>>);

impl Drop for Tarefas {
    fn drop(&mut self) {
        for tarefa in &self.0 {
            tarefa.abort();
        }
    }
}

impl Escuta for EscutaHttp {
    fn escutar(
        &self,
        sessao: Sessao,
        fontes: Vec<(&'static str, &'static str)>,
        canal: Sender<Sinal>,
    ) -> Guarda {
        let tarefas = fontes
            .into_iter()
            .map(|(fonte, caminho)| {
                let api = self.api.clone();
                let sessao = sessao.clone();
                let canal = canal.clone();
                self.tokio
                    .spawn(async move { manter_aberto(api, sessao, fonte, caminho, canal).await })
            })
            .collect();
        Guarda::nova(Tarefas(tarefas))
    }
}

/// O laço de uma fonte: abre, lê até cair, espera e abre de novo.
async fn manter_aberto(
    api: Arc<PosVendaApiHttp>,
    sessao: Sessao,
    fonte: &'static str,
    caminho: &'static str,
    canal: Sender<Sinal>,
) {
    let mut tentativa = 0u32;
    let mut estado = EstadoDaConexao::Conectando;
    loop {
        if canal.send(Sinal::Conexao { fonte, estado }).is_err() {
            return;
        }
        let mut leitor = LeitorSse::default();
        let mut abriu = false;
        let resultado = api
            .escutar(&sessao, caminho, SILENCIO_MAXIMO, |pedaco| {
                for evento in leitor.ler(pedaco) {
                    let sinal = match evento.nome.as_str() {
                        "pronto" => {
                            abriu = true;
                            let _ = canal.send(Sinal::Conexao {
                                fonte,
                                estado: EstadoDaConexao::Conectado,
                            });
                            Sinal::Pronto { fonte }
                        }
                        "sincronizar" => Sinal::Sincronizar { fonte },
                        _ => match serde_json::from_str(&evento.dados) {
                            Ok(dados) => Sinal::Evento { fonte, dados },
                            // O que não é JSON vira releitura, como na fusão do
                            // site: melhor reler à toa que perder o evento.
                            Err(_) => Sinal::Sincronizar { fonte },
                        },
                    };
                    let _ = canal.send(sinal);
                }
            })
            .await;
        if abriu {
            tentativa = 0;
        }
        match resultado {
            Err(DomainError::AcessoRecusado) => {
                let _ = canal.send(Sinal::Conexao {
                    fonte,
                    estado: EstadoDaConexao::Recusado,
                });
                return;
            }
            Err(erro) if erro.to_string().contains("respondeu 403") => {
                crate::telemetria::avisar!("⚠️ [Tempo real] {fonte}: {erro}");
                let _ = canal.send(Sinal::Conexao {
                    fonte,
                    estado: EstadoDaConexao::Recusado,
                });
                return;
            }
            Err(erro) => crate::telemetria::avisar!("⚠️ [Tempo real] {fonte}: {erro}"),
            Ok(()) => {}
        }
        estado = EstadoDaConexao::Reconectando;
        if canal.send(Sinal::Conexao { fonte, estado }).is_err() {
            return;
        }
        tokio::time::sleep(com_sorteio(espera_antes_da_tentativa(tentativa))).await;
        tentativa = tentativa.saturating_add(1);
    }
}

/// A porta de mentira: guarda o canal de cada `escutar`, para o teste mandar
/// o sinal que quiser, e conta as guardas largadas.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    /// Uma escuta aberta: os caminhos, os rótulos das fontes e o canal.
    type Aberta = (Vec<&'static str>, Vec<&'static str>, Sender<Sinal>);

    #[derive(Default)]
    pub struct EscutaDeMentira {
        pub canais: Mutex<Vec<Aberta>>,
        pub largadas: Arc<AtomicUsize>,
    }

    fn fonte(sinal: &Sinal) -> &'static str {
        match sinal {
            Sinal::Conexao { fonte, .. }
            | Sinal::Pronto { fonte }
            | Sinal::Sincronizar { fonte }
            | Sinal::Evento { fonte, .. } => fonte,
        }
    }

    struct Contador(Arc<AtomicUsize>);
    impl Drop for Contador {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    impl EscutaDeMentira {
        /// Manda um sinal pela escuta mais recente **da fonte dele** — o
        /// chatbot e a agenda escutam ao mesmo tempo, cada um as suas.
        pub fn mandar(&self, sinal: Sinal) {
            let canais = self.canais.lock().unwrap();
            let (_, _, canal) = canais
                .iter()
                .rev()
                .find(|(_, fontes, _)| fontes.contains(&fonte(&sinal)))
                .unwrap_or_else(|| panic!("ninguém escuta a fonte {}", fonte(&sinal)));
            canal.send(sinal).unwrap();
        }

        /// Manda pelo canal da escuta que tem `fonte_da_escuta`, seja qual
        /// for a fonte do sinal — para provar que sinal torto não derruba.
        pub fn mandar_pela_escuta_de(&self, fonte_da_escuta: &str, sinal: Sinal) {
            let canais = self.canais.lock().unwrap();
            let (_, _, canal) = canais
                .iter()
                .rev()
                .find(|(_, fontes, _)| fontes.contains(&fonte_da_escuta))
                .expect("ninguém escuta essa fonte");
            canal.send(sinal).unwrap();
        }

        pub fn abertas(&self) -> usize {
            self.canais.lock().unwrap().len()
        }

        /// Os caminhos da escuta mais recente que tem esta fonte.
        pub fn caminhos_da_fonte(&self, fonte: &str) -> Vec<&'static str> {
            self.canais
                .lock()
                .unwrap()
                .iter()
                .rev()
                .find(|(_, fontes, _)| fontes.contains(&fonte))
                .map(|(c, _, _)| c.clone())
                .unwrap_or_default()
        }
    }

    impl Escuta for EscutaDeMentira {
        fn escutar(
            &self,
            _sessao: Sessao,
            fontes: Vec<(&'static str, &'static str)>,
            canal: Sender<Sinal>,
        ) -> Guarda {
            self.canais.lock().unwrap().push((
                fontes.iter().map(|(_, c)| *c).collect(),
                fontes.iter().map(|(f, _)| *f).collect(),
                canal,
            ));
            Guarda::nova(Contador(self.largadas.clone()))
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::sync::mpsc::{channel, Receiver};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn sessao() -> Sessao {
        Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        }
    }

    /// Um servidor que atende cada conexão com a resposta da vez (a última se
    /// repete) e anota o caminho pedido.
    async fn servidor(
        respostas: Vec<&'static [u8]>,
    ) -> (String, Arc<std::sync::Mutex<Vec<String>>>) {
        let ouvinte = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endereco = format!("http://{}", ouvinte.local_addr().unwrap());
        let pedidos = Arc::new(std::sync::Mutex::new(Vec::new()));
        let anotados = pedidos.clone();
        tokio::spawn(async move {
            let mut vez = 0;
            loop {
                let Ok((mut conexao, _)) = ouvinte.accept().await else {
                    return;
                };
                let mut pedido = vec![0u8; 4096];
                let lido = conexao.read(&mut pedido).await.unwrap_or(0);
                let linha = String::from_utf8_lossy(&pedido[..lido])
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                anotados.lock().unwrap().push(linha);
                let resposta = respostas[vez.min(respostas.len() - 1)];
                vez += 1;
                let _ = conexao.write_all(resposta).await;
                let _ = conexao.shutdown().await;
            }
        });
        (endereco, pedidos)
    }

    /// Espera até `n` sinais chegarem (ou o prazo acabar).
    fn colher(canal: &Receiver<Sinal>, n: usize) -> Vec<Sinal> {
        let mut sinais = Vec::new();
        let prazo = std::time::Instant::now() + Duration::from_secs(10);
        while sinais.len() < n && std::time::Instant::now() < prazo {
            if let Ok(sinal) = canal.recv_timeout(Duration::from_millis(50)) {
                sinais.push(sinal);
            }
        }
        sinais
    }

    const FLUXO: &[u8] = b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: 87\r\n\r\n\
event: pronto\ndata: 1\n\n:keep-alive\n\ndata: {\"tipo\":\"mensagem_recebida\",\"contato\":\"55\"}\n\n";

    /// 🔑 O ciclo inteiro contra um servidor de verdade: abre, avisa que está
    /// conectado, entrega o evento, e quando o servidor fecha, **reconecta
    /// sozinho** — é o que o proxy do site faz a cada minuto, e o app tem de
    /// sobreviver a isso sem ninguém clicar.
    #[test]
    fn abre_entrega_o_evento_e_reconecta_quando_o_servidor_fecha() {
        let tokio = tokio::runtime::Runtime::new().unwrap();
        let (base, pedidos) = tokio.block_on(servidor(vec![FLUXO]));
        let escuta = EscutaHttp::nova(
            Arc::new(PosVendaApiHttp::nova(base)),
            tokio.handle().clone(),
        );
        let (envia, recebe) = channel();
        let _guarda = escuta.escutar(sessao(), vec![("whatsapp", "/whatsapp/eventos")], envia);

        let sinais = colher(&recebe, 6);
        use EstadoDaConexao::*;
        assert_eq!(
            sinais[..5],
            [
                Sinal::Conexao {
                    fonte: "whatsapp",
                    estado: Conectando
                },
                Sinal::Conexao {
                    fonte: "whatsapp",
                    estado: Conectado
                },
                Sinal::Pronto { fonte: "whatsapp" },
                Sinal::Evento {
                    fonte: "whatsapp",
                    dados: serde_json::json!({"tipo": "mensagem_recebida", "contato": "55"})
                },
                Sinal::Conexao {
                    fonte: "whatsapp",
                    estado: Reconectando
                },
            ]
        );
        // A reconexão é outra ida ao servidor, no mesmo caminho.
        assert_eq!(
            sinais[5],
            Sinal::Conexao {
                fonte: "whatsapp",
                estado: Reconectando
            }
        );
        let prazo = std::time::Instant::now() + Duration::from_secs(5);
        while pedidos.lock().unwrap().len() < 2 && std::time::Instant::now() < prazo {
            std::thread::sleep(Duration::from_millis(20));
        }
        let pedidos = pedidos.lock().unwrap().clone();
        assert!(pedidos.len() >= 2, "reconectou: {pedidos:?}");
        assert!(pedidos
            .iter()
            .all(|p| p.starts_with("GET /api/v2/whatsapp/eventos ")));
    }

    /// Permissão negada não se cura tentando: `401` e `403` param a fonte, e as
    /// outras continuam.
    #[test]
    fn recusa_para_a_fonte_e_nao_insiste() {
        let tokio = tokio::runtime::Runtime::new().unwrap();
        let (base, pedidos) = tokio.block_on(servidor(vec![
            b"HTTP/1.1 403 Forbidden\r\ncontent-length: 0\r\n\r\n",
        ]));
        let escuta = EscutaHttp::nova(
            Arc::new(PosVendaApiHttp::nova(base)),
            tokio.handle().clone(),
        );
        let (envia, recebe) = channel();
        let _guarda = escuta.escutar(
            sessao(),
            vec![("agenda", "/bookings/agenda/eventos")],
            envia,
        );
        let sinais = colher(&recebe, 2);
        assert_eq!(
            sinais,
            vec![
                Sinal::Conexao {
                    fonte: "agenda",
                    estado: EstadoDaConexao::Conectando
                },
                Sinal::Conexao {
                    fonte: "agenda",
                    estado: EstadoDaConexao::Recusado
                },
            ]
        );
        std::thread::sleep(Duration::from_millis(1500));
        assert!(recebe.try_recv().is_err(), "nada depois da recusa");
        assert_eq!(pedidos.lock().unwrap().len(), 1, "uma tentativa só");
    }

    /// Largar a guarda fecha o fluxo — sair da conta não deixa cano aberto em
    /// nome de quem saiu.
    #[test]
    fn largar_a_guarda_para_tudo() {
        let tokio = tokio::runtime::Runtime::new().unwrap();
        let (base, pedidos) = tokio.block_on(servidor(vec![FLUXO]));
        let escuta = EscutaHttp::nova(
            Arc::new(PosVendaApiHttp::nova(base)),
            tokio.handle().clone(),
        );
        let (envia, recebe) = channel();
        let guarda = escuta.escutar(sessao(), vec![("whatsapp", "/whatsapp/eventos")], envia);
        colher(&recebe, 3);
        drop(guarda);
        std::thread::sleep(Duration::from_millis(100));
        let depois = pedidos.lock().unwrap().len();
        std::thread::sleep(Duration::from_millis(3000));
        assert_eq!(pedidos.lock().unwrap().len(), depois, "ninguém reconectou");
    }

    #[test]
    fn a_espera_dobra_ate_trinta_segundos() {
        let segundos: Vec<u64> = (0..7)
            .map(|n| espera_antes_da_tentativa(n).as_secs())
            .collect();
        assert_eq!(segundos, vec![1, 2, 4, 8, 16, 30, 30]);
        assert_eq!(espera_antes_da_tentativa(u32::MAX).as_secs(), 30);
        let sorteada = com_sorteio(Duration::from_secs(10));
        assert!(sorteada >= Duration::from_secs(10) && sorteada <= Duration::from_secs(13));
    }

    #[test]
    fn o_estado_de_varias_fontes_e_o_da_pior() {
        use EstadoDaConexao::*;
        assert_eq!(EstadoDaConexao::de_todas([Conectado, Conectado]), Conectado);
        assert_eq!(
            EstadoDaConexao::de_todas([Conectado, Reconectando, Conectando]),
            Reconectando
        );
        assert_eq!(EstadoDaConexao::de_todas([Recusado, Conectado]), Recusado);
        assert_eq!(EstadoDaConexao::de_todas([]), Conectando);
    }
}
