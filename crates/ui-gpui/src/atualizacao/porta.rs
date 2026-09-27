//! Descobrir que há versão nova, e instalá-la.
//!
//! ## Por que existe
//!
//! O app não passa por loja nenhuma: ele é baixado de
//! `recordarfotos.com.br/vintageLightbox` e, a partir daí, **ninguém o
//! atualiza**. Sem isto, corrigir um defeito significa pedir a cada fotógrafo
//! que volte ao site e reinstale — e o que acontece de verdade é que a versão
//! velha fica rodando por meses.
//!
//! ## O que garante que a atualização é nossa
//!
//! Cada pacote é assinado com **minisign** (ed25519) por
//! `scripts/empacotar.sh`, e a chave **pública** está compilada aqui dentro
//! ([`CHAVE_PUBLICA`]). O app baixa, confere a assinatura e **só então**
//! instala. Isso é independente da Apple e da Microsoft: se alguém tomar o
//! servidor de download, o pacote trocado não tem assinatura válida e o app o
//! recusa.
//!
//! 🚨 **É a única defesa que existe aqui.** Não há Gatekeeper conferindo por
//! nós — o app é distribuído sem Developer ID por decisão do dono. Trocar
//! `CHAVE_PUBLICA` por uma que não corresponda à chave privada do gerador
//! transforma toda atualização em "assinatura inválida", em silêncio, na
//! máquina do cliente.
//!
//! ## Por que é uma porta
//!
//! Pelo mesmo motivo do [`Acervo`](crate::biblioteca::acervo::Acervo): a tela
//! não pode bloquear, e o teste precisa afirmar **o que a faixa mostra** sem
//! rede. A versão de mentira responde o que o teste mandar.

use std::sync::mpsc::Sender;

/// A chave pública que confere a assinatura dos pacotes.
///
/// É o conteúdo de `empacotamento/chave-publica.txt`, e o par dela vive **fora
/// do repositório**, em `~/.vintagelightbox/atualizacao.key`.
pub const CHAVE_PUBLICA: &str = include_str!("../../../../empacotamento/chave-publica.txt");

/// Onde o app pergunta se há versão nova — a lista, como está no arquivo.
///
/// 🔑 **Um arquivo estático, e não um endpoint.** O `latest.json` traz **todas**
/// as plataformas de uma vez, e quem compara as versões é o próprio app: o
/// updater lê o `version` do manifesto, confronta com a instalada e escolhe a
/// entrada de `platforms` que corresponde a esta máquina.
///
/// 🔑 **Vários endereços, tentados em ordem** (24/set/2026). O R2 vem primeiro
/// porque não depende do GitHub Actions, travado por cobrança desde 17/set; o
/// GitHub Pages fica por último, como espelho. O updater só passa ao seguinte
/// quando o anterior falha — fora do ar, 404, JSON ilegível. A lista mora em
/// `empacotamento/enderecos-de-atualizacao.txt` porque o `lancar-local.sh` lê a
/// mesma, e os dois nunca podem discordar de onde os pacotes estão.
const LISTA_DE_ENDERECOS: &str =
    include_str!("../../../../empacotamento/enderecos-de-atualizacao.txt");

/// Os endereços da lista, na ordem em que o app tenta.
pub fn enderecos() -> Vec<&'static str> {
    LISTA_DE_ENDERECOS
        .lines()
        .map(str::trim)
        .filter(|linha| !linha.is_empty() && !linha.starts_with('#'))
        .collect()
}

/// Quanto a **procura** espera por um endereço antes de tentar o seguinte.
///
/// ⚠️ Só a procura. O mesmo limite no download cortaria um pacote de 30 MB numa
/// conexão lenta de balcão — por isso `instalar` o tira antes de baixar.
const ESPERA_DA_PROCURA: std::time::Duration = std::time::Duration::from_secs(15);

/// Como a versão nova chega a esta máquina.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JeitoDeAtualizar {
    /// O pacote assinado do `latest.json`: baixa, confere e instala.
    Pacote,
    /// O instalador de sempre, rodado pelo app: compila o `main` aqui
    /// ([`super::compilar`]).
    Compilar,
}

/// O que a tela precisa saber sobre uma versão nova.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersaoNova {
    pub versao: String,
    /// O texto do lançamento, quando o manifesto traz um.
    pub notas: Option<String>,
    pub jeito: JeitoDeAtualizar,
    /// O que mudou e por que atualizar (`docs/novidades.json`).
    pub novidades: Option<super::novidades::Novidades>,
}

/// O que a porta responde à tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Aviso {
    /// Há versão nova. `automatico` diz se a compilação começa sozinha
    /// ([`super::compilar::decidir`]).
    Disponivel {
        versao: VersaoNova,
        automatico: bool,
    },
    /// A compilação anunciou uma etapa ("compilando", "instalando em …").
    Progresso(String),
    /// A instalação terminou; falta reabrir o app.
    Instalada(String),
    /// Não deu — e a tela mostra isso **sem** derrubar nada.
    Falhou(String),
    /// A procura **pedida** não achou nada: esta é a versão mais recente.
    ///
    /// 🔑 Só a procura pedida responde isto. Na da abertura, "nada novo" é
    /// silêncio — ninguém perguntou.
    EmDia(String),
    /// A procura **pedida** não conseguiu perguntar a servidor nenhum.
    SemResposta(String),
}

/// Quem sabe procurar e instalar versão nova.
pub trait Atualizador: Send + Sync + 'static {
    /// Pergunta ao servidor e **devolve na hora**. A resposta chega pelo canal.
    ///
    /// ⚠️ **Sem `Result`.** Um servidor fora do ar é o app seguir na versão que
    /// tem — que é o desfecho certo. Um `?` aqui faria a falha do acessório
    /// (avisar de versão nova) derrubar o principal (revelar fotos).
    ///
    /// Quando não há nada novo, **nada é enviado pelo canal**: silêncio é a
    /// resposta normal, e ela não merece interromper ninguém.
    ///
    /// `pedida` é o operador que clicou em "Verificar atualizações": aí o
    /// silêncio vira [`Aviso::EmDia`], e a falha vira [`Aviso::SemResposta`] —
    /// houve um clique esperando resposta.
    fn procurar(&self, canal: Sender<Aviso>, pedida: bool);

    /// Baixa, confere a assinatura e instala. A resposta chega pelo canal.
    fn instalar(&self, canal: Sender<Aviso>);

    /// Roda o instalador, que compila o `main` nesta máquina. As etapas
    /// chegam como [`Aviso::Progresso`]; o fim, como `Instalada` ou `Falhou`.
    fn compilar(&self, versao: String, canal: Sender<Aviso>);
}

pub use real::AtualizadorDaWeb;

mod real {
    use super::*;

    /// O atualizador de verdade — `cargo-packager-updater`, o updater do
    /// `cargo-packager` que monta os instaladores.
    ///
    /// 🔑 **Ele não guarda o `Update` que a procura achou.** O `Update` carrega
    /// uma `Config` e um `HeaderMap`, e a porta atravessa threads; guardá-lo
    /// obrigaria a struct inteira a ser `Sync` por causa de um valor que custa
    /// um GET para obter de novo. `instalar` pergunta outra vez, de propósito —
    /// e de quebra pega o caso em que o lançamento foi retirado entre o aviso e
    /// o clique.
    pub struct AtualizadorDaWeb {
        versao_atual: String,
    }

    impl AtualizadorDaWeb {
        pub fn novo(versao_atual: impl Into<String>) -> Self {
            Self {
                versao_atual: versao_atual.into(),
            }
        }

        fn config() -> cargo_packager_updater::Config {
            cargo_packager_updater::Config {
                endpoints: enderecos()
                    .into_iter()
                    .map(|e| {
                        e.parse()
                            .expect("os endereços de atualização são literais e válidos")
                    })
                    .collect(),
                pubkey: CHAVE_PUBLICA.trim().to_string(),
                ..Default::default()
            }
        }

        /// A consulta, já fora da thread da interface.
        ///
        /// Associada e não método: quem a chama é uma thread nova, que não
        /// consegue levar `&self` consigo — só a versão, que é um `String`.
        fn consultar(versao: &str) -> Result<Option<cargo_packager_updater::Update>, String> {
            let atual = versao
                .parse()
                .map_err(|e| format!("versão instalada ilegível: {e}"))?;
            cargo_packager_updater::UpdaterBuilder::new(atual, Self::config())
                .timeout(ESPERA_DA_PROCURA)
                .build()
                .and_then(|updater| updater.check())
                .map_err(|e| format!("não consegui perguntar ao servidor: {e}"))
        }
    }

    impl Atualizador for AtualizadorDaWeb {
        fn procurar(&self, canal: Sender<Aviso>, pedida: bool) {
            let atual = self.versao_atual.clone();
            // 🚨 Uma thread do sistema, e **não** `tokio::spawn`. O
            // `check_update` é bloqueante e monta um runtime próprio por dentro
            // (reqwest blocking); chamá-lo de dentro de um runtime do tokio
            // entra em pânico com *Cannot drop a runtime in a context where
            // blocking is not allowed*. É a única porta da casa que não usa a
            // `Handle` — por isso, e não por esquecimento.
            std::thread::spawn(move || {
                use super::super::compilar;
                use super::super::novidades::{self, JeitoDaInstalacao};
                let jeito = novidades::jeito_desta_instalacao();
                // Algum servidor respondeu? Só a procura pedida usa isto, para
                // não dizer "está em dia" sem ter perguntado a ninguém.
                let mut respondeu = false;
                let mut motivo_da_falha = None;
                // A instalação compilada nunca recebe pacote: nem pergunta.
                let pacote = if jeito == JeitoDaInstalacao::Compilado {
                    None
                } else {
                    match AtualizadorDaWeb::consultar(&atual) {
                        Ok(nova) => {
                            respondeu = true;
                            nova.map(|nova| VersaoNova {
                                versao: nova.version.clone(),
                                notas: nova.body.clone(),
                                jeito: JeitoDeAtualizar::Pacote,
                                novidades: None,
                            })
                        }
                        // 🔑 A falha da procura **da abertura** não vai para a
                        // tela. Ninguém pediu nada; avisar "não consegui checar
                        // atualização" a cada abertura sem rede é ruído puro.
                        Err(motivo) => {
                            eprintln!("[atualização] {motivo}");
                            motivo_da_falha = Some(motivo);
                            None
                        }
                    }
                };
                let main = if jeito == JeitoDaInstalacao::Desenvolvimento {
                    None
                } else {
                    novidades::buscar()
                };
                respondeu |= main.is_some();
                let agora = compilar::agora();
                let casa = compilar::casa();
                let memoria = casa
                    .as_ref()
                    .map(|c| compilar::ler_memoria(&c.join("atualizacao.json")))
                    .unwrap_or_default();
                let ocupado = casa
                    .as_ref()
                    .is_some_and(|c| compilar::ocupado(&c.join("atualizando.trava"), agora));
                // Silêncio é a resposta normal: nada novo, nada na tela.
                match compilar::decidir(jeito, &atual, pacote, main, &memoria, ocupado, agora) {
                    compilar::Decisao::Avisar { versao, automatico } => {
                        let _ = canal.send(Aviso::Disponivel { versao, automatico });
                    }
                    _ if !pedida => {}
                    _ if respondeu => {
                        let _ = canal.send(Aviso::EmDia(atual));
                    }
                    _ => {
                        let _ =
                            canal.send(Aviso::SemResposta(motivo_da_falha.unwrap_or_else(|| {
                                "nenhum servidor de atualização respondeu".into()
                            })));
                    }
                }
            });
        }

        fn instalar(&self, canal: Sender<Aviso>) {
            let atual = self.versao_atual.clone();
            std::thread::spawn(move || {
                let aviso = match AtualizadorDaWeb::consultar(&atual) {
                    Ok(Some(mut nova)) => {
                        let versao = nova.version.clone();
                        // O limite era da procura; o download leva o tempo que
                        // a conexão do balcão precisar.
                        nova.timeout = None;
                        // Baixa, **confere a assinatura** e instala. A conferência
                        // é do próprio updater, contra `CHAVE_PUBLICA`.
                        match nova.download_and_install() {
                            Ok(()) => Aviso::Instalada(versao),
                            Err(e) => Aviso::Falhou(format!("a instalação falhou: {e}")),
                        }
                    }
                    Ok(None) => Aviso::Falhou("a versão nova sumiu do servidor".into()),
                    Err(motivo) => Aviso::Falhou(motivo),
                };
                let _ = canal.send(aviso);
            });
        }

        fn compilar(&self, versao: String, canal: Sender<Aviso>) {
            std::thread::spawn(move || {
                use super::super::compilar::{self, Andamento};
                compilar::rodar(&versao, &|andamento| {
                    let aviso = match andamento {
                        Andamento::Etapa(etapa) => Aviso::Progresso(etapa),
                        Andamento::Instalada(versao) => Aviso::Instalada(versao),
                        Andamento::Falhou(motivo) => Aviso::Falhou(motivo),
                    };
                    let _ = canal.send(aviso);
                });
            });
        }
    }

    impl AtualizadorDaWeb {
        /// Reabre o app depois de instalar.
        ///
        /// ⚠️ Só o Windows relança sozinho (o instalador NSIS faz isso). No
        /// macOS e no Linux o binário foi trocado no disco e **este processo
        /// continua sendo o antigo** — quem não reabrir segue rodando a versão
        /// velha achando que atualizou.
        pub fn reabrir() {
            let caminho = std::env::current_exe().ok();
            #[cfg(target_os = "macos")]
            if let Some(caminho) = &caminho {
                // O executável mora em `VintageLightbox.app/Contents/MacOS/`;
                // quem se abre é o **bundle**, três níveis acima, senão o app
                // sobe sem ícone, sem menu e sem Info.plist.
                if let Some(bundle) = caminho.ancestors().nth(3) {
                    let _ = std::process::Command::new("open")
                        .arg("-n")
                        .arg(bundle)
                        .spawn();
                    std::process::exit(0);
                }
            }
            #[cfg(not(target_os = "macos"))]
            if let Some(caminho) = &caminho {
                let _ = std::process::Command::new(caminho).spawn();
                std::process::exit(0);
            }
            let _ = caminho;
        }
    }
}

/// O atualizador dos testes: responde o que lhe mandarem, e conta os pedidos.
#[cfg(test)]
pub mod mentira {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct AtualizadorDeMentira {
        /// O que `procurar` envia. `None` é o silêncio de "nada novo".
        pub resposta: Mutex<Option<Aviso>>,
        /// O que `instalar` envia.
        pub desfecho: Mutex<Option<Aviso>>,
        pub procuras: Mutex<usize>,
        pub instalacoes: Mutex<usize>,
        /// As versões que `compilar` recebeu, em ordem.
        pub compilacoes: Mutex<Vec<String>>,
        /// O que `compilar` envia, em ordem (etapas e o fim).
        pub andamento_da_compilacao: Mutex<Vec<Aviso>>,
    }

    impl AtualizadorDeMentira {
        pub fn com_versao(versao: &str) -> Self {
            Self {
                resposta: Mutex::new(Some(Aviso::Disponivel {
                    versao: VersaoNova {
                        versao: versao.into(),
                        notas: None,
                        jeito: JeitoDeAtualizar::Pacote,
                        novidades: None,
                    },
                    automatico: false,
                })),
                desfecho: Mutex::new(Some(Aviso::Instalada(versao.into()))),
                ..Default::default()
            }
        }
    }

    impl Atualizador for AtualizadorDeMentira {
        fn procurar(&self, canal: Sender<Aviso>, pedida: bool) {
            *self.procuras.lock().expect("as procuras") += 1;
            match self.resposta.lock().expect("a resposta").clone() {
                Some(aviso) => {
                    let _ = canal.send(aviso);
                }
                // O mesmo contrato do de verdade: a procura pedida não fica
                // em silêncio.
                None if pedida => {
                    let _ = canal.send(Aviso::EmDia(env!("CARGO_PKG_VERSION").into()));
                }
                None => {}
            }
        }

        fn instalar(&self, canal: Sender<Aviso>) {
            *self.instalacoes.lock().expect("as instalações") += 1;
            if let Some(aviso) = self.desfecho.lock().expect("o desfecho").clone() {
                let _ = canal.send(aviso);
            }
        }

        fn compilar(&self, versao: String, canal: Sender<Aviso>) {
            self.compilacoes
                .lock()
                .expect("as compilações")
                .push(versao);
            for aviso in self
                .andamento_da_compilacao
                .lock()
                .expect("o andamento")
                .clone()
            {
                let _ = canal.send(aviso);
            }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn todo_endereco_da_lista_e_uma_url_de_manifesto() {
        let lista = enderecos();
        assert!(
            !lista.is_empty(),
            "sem endereço, ninguém recebe atualização"
        );
        for endereco in &lista {
            let url: cargo_packager_updater::reqwest::Url =
                endereco.parse().expect("endereço ilegível");
            assert_eq!(url.scheme(), "https", "{endereco}");
            assert!(url.path().ends_with("/latest.json"), "{endereco}");
        }
    }

    /// 🚨 O 0.1.9 e anteriores só conhecem o Pages. Tirá-lo da lista antes de
    /// todo balcão ter passado por uma versão que conhece o R2 é deixar esses
    /// balcões presos, e em silêncio.
    #[test]
    fn o_pages_continua_na_lista_para_quem_ainda_nao_conhece_o_r2() {
        assert!(enderecos().contains(&"https://alexkads.github.io/VintageLightbox/latest.json"));
    }

    #[test]
    fn o_r2_quando_ligado_vem_antes_do_github() {
        let lista = enderecos();
        let r2 = lista.iter().position(|e| e.contains(".r2.dev/"));
        let pages = lista.iter().position(|e| e.contains("github.io"));
        if let (Some(r2), Some(pages)) = (r2, pages) {
            assert!(r2 < pages, "o R2 é o principal; o Pages é espelho");
        }
    }

    /// Contra a rede de verdade: o **primeiro** endereço da lista (o R2) acha
    /// uma versão, baixa o pacote desta plataforma e a assinatura confere.
    /// `download` confere a assinatura antes de devolver; `install` não roda.
    ///
    /// `cargo test -p ui-gpui --lib o_primeiro_endereco -- --ignored`
    #[test]
    #[ignore = "usa a rede e baixa ~30 MB"]
    fn o_primeiro_endereco_serve_um_pacote_com_assinatura_valida() {
        let primeiro = enderecos()[0];
        let config = cargo_packager_updater::Config {
            endpoints: vec![primeiro.parse().expect("endereço")],
            pubkey: CHAVE_PUBLICA.trim().to_string(),
            ..Default::default()
        };
        let nova = cargo_packager_updater::check_update("0.0.1".parse().unwrap(), config)
            .expect("o manifesto responde")
            .expect("há versão para esta plataforma");
        assert!(nova
            .download_url
            .as_str()
            .starts_with(primeiro.trim_end_matches("latest.json")));
        nova.download().expect("baixou e a assinatura confere");
    }
}
