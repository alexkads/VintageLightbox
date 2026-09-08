//! Onde o par de tokens dorme: o **chaveiro do sistema**, nunca um arquivo.
//!
//! # Por que o chaveiro
//!
//! O refresh token do app vale quinze dias e abre a conta do estúdio inteira.
//! Num JSON ao lado do catálogo ele entraria em todo backup, apareceria em
//! qualquer `cat` distraído e viajaria junto se a pasta do catálogo fosse
//! copiada para outra máquina. No Keychain ele fica preso ao usuário do sistema,
//! cifrado com a sessão dele, e um `Time Machine` restaurado noutro Mac não o
//! entrega.
//!
//! É a mesma decisão que `pos_venda.rs` (a configuração) já registrava ao
//! recusar guardar senha — só que agora existe onde guardar direito.
//!
//! # O formato é JSON, e é detalhe
//!
//! O chaveiro guarda uma cadeia de caracteres; o que entra nela é o par de
//! tokens serializado. Ninguém lê esse JSON além daqui, e ele nunca toca o
//! disco.

use domain::services::pos_venda::{CofreDeSessao, Sessao};
use keyring::Entry;
use serde::{Deserialize, Serialize};

/// O nome sob o qual o item aparece no "Acesso às Chaves" do macOS.
const SERVICO: &str = "br.com.recordarfotos.vintagelightbox";
/// A conta dentro do serviço. Uma só: o app opera um estúdio por máquina.
const CONTA: &str = "pos-venda";

#[derive(Serialize, Deserialize)]
struct SessaoGuardada {
    access_token: String,
    refresh_token: String,
    access_vence_em: i64,
    refresh_vence_em: i64,
}

pub struct CofreDoSistema {
    servico: String,
    conta: String,
}

impl Default for CofreDoSistema {
    fn default() -> Self {
        Self::novo()
    }
}

impl CofreDoSistema {
    pub fn novo() -> Self {
        Self {
            servico: SERVICO.to_string(),
            conta: CONTA.to_string(),
        }
    }

    fn entrada(&self) -> Option<Entry> {
        match Entry::new(&self.servico, &self.conta) {
            Ok(e) => Some(e),
            Err(erro) => {
                // Sem chaveiro o app continua inteiro — só volta a pedir
                // autorização na próxima abertura. Ver `CofreDeSessao`.
                eprintln!("⚠️  Chaveiro indisponível: {erro}");
                None
            }
        }
    }
}

impl CofreDeSessao for CofreDoSistema {
    fn guardar(&self, sessao: &Sessao) {
        let Some(entrada) = self.entrada() else {
            return;
        };
        let guardada = SessaoGuardada {
            access_token: sessao.access_token.clone(),
            refresh_token: sessao.refresh_token.clone(),
            access_vence_em: sessao.access_vence_em,
            refresh_vence_em: sessao.refresh_vence_em,
        };
        let Ok(texto) = serde_json::to_string(&guardada) else {
            return;
        };
        if let Err(erro) = entrada.set_password(&texto) {
            eprintln!("⚠️  Não foi possível guardar a sessão no chaveiro: {erro}");
        }
    }

    fn ler(&self) -> Option<Sessao> {
        let texto = self.entrada()?.get_password().ok()?;
        // Formato ilegível é chaveiro com resto de uma versão anterior: vale o
        // mesmo que não ter nada, e autorizar de novo resolve.
        let g: SessaoGuardada = serde_json::from_str(&texto).ok()?;
        Some(Sessao {
            access_token: g.access_token,
            refresh_token: g.refresh_token,
            access_vence_em: g.access_vence_em,
            refresh_vence_em: g.refresh_vence_em,
        })
    }

    fn esquecer(&self) {
        let Some(entrada) = self.entrada() else {
            return;
        };
        // `NoEntry` é o desfecho desejado de quem manda esquecer: já não há nada.
        match entrada.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(erro) => eprintln!("⚠️  Não foi possível apagar a sessão do chaveiro: {erro}"),
        }
    }
}

/// O cofre dos testes e do modo sem chaveiro: lembra enquanto o app está aberto.
#[derive(Default)]
pub struct CofreEmMemoria {
    guardada: std::sync::Mutex<Option<Sessao>>,
}

impl CofreDeSessao for CofreEmMemoria {
    fn guardar(&self, sessao: &Sessao) {
        *self.guardada.lock().expect("o cofre") = Some(sessao.clone());
    }

    fn ler(&self) -> Option<Sessao> {
        self.guardada.lock().expect("o cofre").clone()
    }

    fn esquecer(&self) {
        *self.guardada.lock().expect("o cofre") = None;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn sessao() -> Sessao {
        Sessao {
            access_token: "acesso".into(),
            refresh_token: "renovacao".into(),
            access_vence_em: 1_000,
            refresh_vence_em: 2_000,
        }
    }

    #[test]
    fn o_cofre_em_memoria_devolve_o_que_guardou_e_esquece_quando_mandado() {
        let cofre = CofreEmMemoria::default();
        assert_eq!(cofre.ler(), None);

        cofre.guardar(&sessao());
        assert_eq!(cofre.ler(), Some(sessao()));

        cofre.esquecer();
        assert_eq!(cofre.ler(), None);
    }

    /// 🚨 O que vai para o chaveiro é o par inteiro — se um dia alguém trocar o
    /// formato e esquecer o `refresh_token`, a sessão relida morre em quinze
    /// minutos e ninguém entende por quê.
    #[test]
    fn o_formato_guardado_leva_os_dois_tokens_e_os_dois_prazos() {
        let s = sessao();
        let texto = serde_json::to_string(&SessaoGuardada {
            access_token: s.access_token.clone(),
            refresh_token: s.refresh_token.clone(),
            access_vence_em: s.access_vence_em,
            refresh_vence_em: s.refresh_vence_em,
        })
        .unwrap();

        let volta: SessaoGuardada = serde_json::from_str(&texto).unwrap();
        assert_eq!(volta.refresh_token, "renovacao");
        assert_eq!(volta.refresh_vence_em, 2_000);
    }
}
