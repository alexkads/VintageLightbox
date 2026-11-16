//! 🎙️ A vigia das vozes: o que fala sem ninguém clicar — os alertas da API e
//! as quedas e voltas da conexão. Roda no laço de um segundo da raiz
//! (`ouvir_os_anuncios`).
//!
//! - **Alertas da API** (`telemetria::tomar_alertas`): o servidor só os manda a
//!   administradores; o `codigo` escolhe a frase gravada.
//! - **Conexão** (`sincronizacao::Situacao`): o site parou de responder → um
//!   teste de 3 s numa thread à parte diz se é **este computador sem internet**
//!   ou **o servidor fora**; voltou → "a conexão voltou"; a fila que esperava a
//!   queda esvaziou → "tudo enviado". Por que aqui, e não no servidor: quando o
//!   servidor cai, aviso nenhum dele chega — só o app sabe.

use std::sync::mpsc::{channel, Receiver};
use std::sync::Mutex;
use std::time::Duration;

use gpui_kit::Context;

use super::Aplicativo;
use crate::sons::Evento;

/// O que mudou na conexão desde o último passo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mudanca {
    /// O site parou de responder: falta descobrir se é a internet.
    Caiu,
    Voltou,
    /// A fila que esperava a queda esvaziou.
    TudoEnviado,
}

/// As transições, sem relógio nem rede: o teste diz o que sai de cada passo.
#[derive(Debug, Default)]
pub struct VigiaDaConexao {
    /// `None` = ainda não viu nenhum passo (a abertura não fala).
    sem_rede: Option<bool>,
    /// Desde a última queda há o que enviar: o "tudo enviado" vale a pena.
    esperando_envio: bool,
}

impl VigiaDaConexao {
    pub fn passo(&mut self, sem_rede: bool, pendentes: usize) -> Option<Mudanca> {
        let antes = self.sem_rede.replace(sem_rede);
        // Algo esperou a rede durante a queda: quando a fila esvaziar, vale dizer.
        if sem_rede && pendentes > 0 {
            self.esperando_envio = true;
        }
        match antes {
            Some(false) if sem_rede => return Some(Mudanca::Caiu),
            Some(true) if !sem_rede => return Some(Mudanca::Voltou),
            _ => {}
        }
        if self.esperando_envio && !sem_rede && pendentes == 0 {
            self.esperando_envio = false;
            return Some(Mudanca::TudoEnviado);
        }
        None
    }
}

/// Há internet? Uma conexão TCP de 3 s a um endereço fixo (sem DNS, que pode
/// ser exatamente o que falhou).
fn ha_internet() -> bool {
    let destinos = ["1.1.1.1:443", "8.8.8.8:443"];
    destinos.iter().any(|d| {
        d.parse().ok().is_some_and(|a| {
            std::net::TcpStream::connect_timeout(&a, Duration::from_secs(3)).is_ok()
        })
    })
}

struct Estado {
    vigia: VigiaDaConexao,
    /// O teste de internet em curso, depois de uma queda.
    teste: Option<Receiver<bool>>,
}

static ESTADO: Mutex<Option<Estado>> = Mutex::new(None);

/// O evento sonoro de um alerta da API, pelo `codigo`.
pub fn evento_do_alerta(codigo: &str) -> Evento {
    match codigo {
        "memoria" => Evento::AlertaDeMemoria,
        "normal" => Evento::ApiNormalizada,
        _ => Evento::AlertaDaApi,
    }
}

impl Aplicativo {
    /// Uma volta da vigia (a cada segundo).
    pub(super) fn vigiar_as_vozes(&mut self, cx: &mut Context<Self>) {
        for (codigo, titulo, detalhe) in crate::telemetria::tomar_alertas() {
            crate::sons::soar(evento_do_alerta(&codigo), &titulo, &detalhe, cx);
        }

        let Some(fila) = crate::sincronizacao::a_do_app() else {
            return;
        };
        let situacao = fila.situacao();
        let mut guarda = ESTADO.lock().unwrap_or_else(|e| e.into_inner());
        let estado = guarda.get_or_insert_with(|| Estado {
            vigia: VigiaDaConexao::default(),
            teste: None,
        });

        // O teste de internet terminou: agora se sabe o que falar.
        if let Some(resposta) = estado.teste.as_ref().and_then(|r| r.try_recv().ok()) {
            estado.teste = None;
            let evento = if resposta {
                Evento::ServidorFora
            } else {
                Evento::SemInternet
            };
            crate::sons::soar(evento, "", "", cx);
        }

        match estado
            .vigia
            .passo(situacao.sem_rede_desde.is_some(), situacao.pendentes)
        {
            Some(Mudanca::Caiu) => {
                let (envia, recebe) = channel();
                std::thread::spawn(move || {
                    let _ = envia.send(ha_internet());
                });
                estado.teste = Some(recebe);
            }
            Some(Mudanca::Voltou) => {
                estado.teste = None;
                crate::sons::soar(Evento::ConexaoVoltou, "", "", cx);
            }
            Some(Mudanca::TudoEnviado) => crate::sons::soar(Evento::EnvioConcluido, "", "", cx),
            None => {}
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_abertura_nao_fala_mesmo_sem_rede() {
        let mut v = VigiaDaConexao::default();
        assert_eq!(v.passo(true, 3), None);
    }

    #[test]
    fn caiu_voltou_e_esvaziou_cada_um_uma_vez() {
        let mut v = VigiaDaConexao::default();
        assert_eq!(v.passo(false, 0), None);
        assert_eq!(v.passo(true, 0), Some(Mudanca::Caiu));
        assert_eq!(v.passo(true, 2), None, "segue fora: não repete");
        assert_eq!(v.passo(false, 2), Some(Mudanca::Voltou));
        assert_eq!(v.passo(false, 1), None, "ainda subindo");
        assert_eq!(v.passo(false, 0), Some(Mudanca::TudoEnviado));
        assert_eq!(v.passo(false, 0), None, "não repete");
    }

    #[test]
    fn caiu_sem_nada_na_fila_nao_diz_tudo_enviado() {
        let mut v = VigiaDaConexao::default();
        v.passo(false, 0);
        assert_eq!(v.passo(true, 0), Some(Mudanca::Caiu));
        assert_eq!(v.passo(false, 0), Some(Mudanca::Voltou));
        assert_eq!(
            v.passo(false, 0),
            None,
            "nada esperou: não há o que anunciar"
        );
    }

    #[test]
    fn com_rede_a_fila_que_esvazia_nao_fala() {
        let mut v = VigiaDaConexao::default();
        v.passo(false, 3);
        assert_eq!(v.passo(false, 0), None, "sem queda, esvaziar é o normal");
    }

    #[test]
    fn os_codigos_viram_os_eventos_certos() {
        assert_eq!(evento_do_alerta("falha"), Evento::AlertaDaApi);
        assert_eq!(evento_do_alerta("memoria"), Evento::AlertaDeMemoria);
        assert_eq!(evento_do_alerta("normal"), Evento::ApiNormalizada);
        assert_eq!(evento_do_alerta("?"), Evento::AlertaDaApi);
    }
}
