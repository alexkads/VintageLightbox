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

/// Onde o app pergunta se há versão nova.
///
/// 🔑 **Um arquivo estático, e não um endpoint.** O `latest.json` traz **todas**
/// as plataformas de uma vez, e quem compara as versões é o próprio app: o
/// updater lê o `version` do manifesto, confronta com a instalada e escolhe a
/// entrada de `platforms` que corresponde a esta máquina.
///
/// Isso é o que permite o projeto não ter servidor nenhum. Antes isto apontava
/// para uma rota do `recordarfotos.com.br`, que respondia `204` quando não havia
/// novidade; a rota saiu quando a distribuição foi para o GitHub, e com ela saiu
/// a última credencial que o lançamento precisava (7/set/2026).
///
/// ⚠️ **O GitHub Pages serve com cache curto, mas serve com cache.** Um
/// lançamento pode levar alguns minutos para chegar a todo mundo — o que é
/// irrelevante para algo que o app consulta uma vez por abertura.
pub const ENDERECO: &str = "https://alexkads.github.io/VintageLightbox/latest.json";

/// O que a tela precisa saber sobre uma versão nova.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersaoNova {
    pub versao: String,
    /// O texto do lançamento, quando o manifesto traz um.
    pub notas: Option<String>,
}

/// O que a porta responde à tela.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Aviso {
    /// Há versão nova esperando decisão.
    Disponivel(VersaoNova),
    /// A instalação terminou; falta reabrir o app.
    Instalada(String),
    /// Não deu — e a tela mostra isso **sem** derrubar nada.
    Falhou(String),
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
    fn procurar(&self, canal: Sender<Aviso>);

    /// Baixa, confere a assinatura e instala. A resposta chega pelo canal.
    fn instalar(&self, canal: Sender<Aviso>);
}

pub use real::AtualizadorDaWeb;

mod real {
    use super::*;

    /// O atualizador de verdade — `cargo-packager-updater`, que é o updater do
    /// Tauri extraído para servir app que não é Tauri.
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
                endpoints: vec![ENDERECO
                    .parse()
                    .expect("o endereço de atualização é literal e válido")],
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
            cargo_packager_updater::check_update(atual, Self::config())
                .map_err(|e| format!("não consegui perguntar ao servidor: {e}"))
        }
    }

    impl Atualizador for AtualizadorDaWeb {
        fn procurar(&self, canal: Sender<Aviso>) {
            let atual = self.versao_atual.clone();
            // 🚨 Uma thread do sistema, e **não** `tokio::spawn`. O
            // `check_update` é bloqueante e monta um runtime próprio por dentro
            // (reqwest blocking); chamá-lo de dentro de um runtime do tokio
            // entra em pânico com *Cannot drop a runtime in a context where
            // blocking is not allowed*. É a única porta da casa que não usa a
            // `Handle` — por isso, e não por esquecimento.
            std::thread::spawn(move || {
                match AtualizadorDaWeb::consultar(&atual) {
                    // Silêncio é a resposta normal: nada novo, nada na tela.
                    Ok(None) => {}
                    Ok(Some(nova)) => {
                        let _ = canal.send(Aviso::Disponivel(VersaoNova {
                            versao: nova.version.clone(),
                            notas: nova.body.clone(),
                        }));
                    }
                    // 🔑 A falha da **procura** não vai para a tela. Ninguém
                    // pediu nada; avisar "não consegui checar atualização" a
                    // cada abertura sem rede é ruído puro. A falha da
                    // **instalação** vai, porque ali houve um clique esperando
                    // resposta.
                    Err(motivo) => eprintln!("[atualização] {motivo}"),
                }
            });
        }

        fn instalar(&self, canal: Sender<Aviso>) {
            let atual = self.versao_atual.clone();
            std::thread::spawn(move || {
                let aviso = match AtualizadorDaWeb::consultar(&atual) {
                    Ok(Some(nova)) => {
                        let versao = nova.version.clone();
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
    }

    impl AtualizadorDeMentira {
        pub fn com_versao(versao: &str) -> Self {
            Self {
                resposta: Mutex::new(Some(Aviso::Disponivel(VersaoNova {
                    versao: versao.into(),
                    notas: None,
                }))),
                desfecho: Mutex::new(Some(Aviso::Instalada(versao.into()))),
                ..Default::default()
            }
        }
    }

    impl Atualizador for AtualizadorDeMentira {
        fn procurar(&self, canal: Sender<Aviso>) {
            *self.procuras.lock().expect("as procuras") += 1;
            if let Some(aviso) = self.resposta.lock().expect("a resposta").clone() {
                let _ = canal.send(aviso);
            }
        }

        fn instalar(&self, canal: Sender<Aviso>) {
            *self.instalacoes.lock().expect("as instalações") += 1;
            if let Some(aviso) = self.desfecho.lock().expect("o desfecho").clone() {
                let _ = canal.send(aviso);
            }
        }
    }
}
