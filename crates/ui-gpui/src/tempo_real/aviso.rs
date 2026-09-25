//! O aviso do sistema operacional — a `Notification` do navegador, para quando
//! o app não está na frente.
//!
//! 🔑 **Não depende da bandeja.** No GNOME sem a extensão AppIndicator não há
//! ícone de bandeja (`bandeja.rs`), mas o servidor de notificações existe: o
//! aviso vai por D-Bus direto a ele.
//!
//! | Sistema | Aparece | O clique volta ao app |
//! |---|---|---|
//! | Linux (GNOME, KDE) | sim, por D-Bus | sim — a ação `default` |
//! | Windows 11 | sim, toast | não (o `notify-rust` não informa) |
//! | macOS | sim, Central de Notificações | não |
//!
//! Onde o clique não volta, o aviso ainda cumpre o papel principal, que é o
//! operador saber que tem alguém esperando.

use std::collections::HashMap;
use std::sync::mpsc::Sender;
use std::sync::Mutex;

/// Um aviso. `destino` diz o que abrir quando o operador clica, e aviso novo
/// com o mesmo destino **substitui** o anterior — a `tag` da `Notification` do
/// site: três mensagens seguidas da mesma pessoa são um aviso, não três.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Aviso {
    pub titulo: String,
    pub corpo: String,
    pub destino: String,
}

pub trait Avisador: Send + Sync + 'static {
    /// Mostra o aviso. Devolve na hora; se o operador clicar, o `destino`
    /// volta por `cliques`.
    fn avisar(&self, aviso: Aviso, cliques: Sender<String>);
}

/// O aviso do sistema, pelo `notify-rust`.
#[derive(Default)]
pub struct AvisoDoSistema {
    /// O id que o servidor de notificações deu ao último aviso de cada
    /// destino — é o que permite substituir em vez de empilhar.
    #[cfg_attr(not(all(unix, not(target_os = "macos"))), allow(dead_code))]
    ultimos: Mutex<HashMap<String, u32>>,
}

impl Avisador for AvisoDoSistema {
    fn avisar(&self, aviso: Aviso, cliques: Sender<String>) {
        let mut notificacao = notify_rust::Notification::new();
        notificacao
            .appname("VintageLightbox")
            .summary(&aviso.titulo)
            .body(&aviso.corpo);

        #[cfg(all(unix, not(target_os = "macos")))]
        {
            notificacao.action("default", "Abrir");
            if let Some(id) = self.ultimos.lock().unwrap().get(&aviso.destino) {
                notificacao.id(*id);
            }
            let handle = match notificacao.show() {
                Ok(handle) => handle,
                Err(erro) => {
                    eprintln!("⚠️ [Aviso] o sistema recusou a notificação: {erro}");
                    return;
                }
            };
            self.ultimos
                .lock()
                .unwrap()
                .insert(aviso.destino.clone(), handle.id());
            // Esperar o clique bloqueia até o aviso fechar — numa thread
            // própria, que termina junto com ele.
            let destino = aviso.destino;
            std::thread::spawn(move || {
                handle.wait_for_action(|acao| {
                    if acao == "default" {
                        let _ = cliques.send(destino);
                    }
                });
            });
        }

        #[cfg(not(all(unix, not(target_os = "macos"))))]
        {
            let _ = cliques;
            // O `show` do macOS espera o sistema responder: fora da thread da
            // interface.
            std::thread::spawn(move || {
                if let Err(erro) = notificacao.show() {
                    eprintln!("⚠️ [Aviso] o sistema recusou a notificação: {erro}");
                }
            });
        }
    }
}

/// O avisador de mentira: guarda o que foi avisado, e o canal do clique para o
/// teste clicar.
#[cfg(test)]
pub mod mentira {
    use super::*;

    #[derive(Default)]
    pub struct AvisadorDeMentira {
        pub avisados: Mutex<Vec<(Aviso, Sender<String>)>>,
    }

    impl AvisadorDeMentira {
        pub fn avisos(&self) -> Vec<Aviso> {
            self.avisados
                .lock()
                .unwrap()
                .iter()
                .map(|(a, _)| a.clone())
                .collect()
        }

        /// O operador clicou no último aviso.
        pub fn clicar_no_ultimo(&self) {
            let avisados = self.avisados.lock().unwrap();
            let (aviso, canal) = avisados.last().expect("nenhum aviso para clicar");
            canal.send(aviso.destino.clone()).unwrap();
        }
    }

    impl Avisador for AvisadorDeMentira {
        fn avisar(&self, aviso: Aviso, cliques: Sender<String>) {
            self.avisados.lock().unwrap().push((aviso, cliques));
        }
    }
}
