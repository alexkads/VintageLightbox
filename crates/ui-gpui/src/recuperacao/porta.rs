//! A porta da recuperação: listar os cartões e rodar a varredura elevada.
//!
//! Como as outras portas da casa, **nunca devolve `Result` para a tela**: tudo
//! chega como [`Recado`] pelo canal, inclusive a falha e a desistência.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::time::Duration;

use domain::recuperacao::{CartaoBruto, CartoesBrutos};
use infrastructure::recuperacao::{ler_andamento, ARQUIVO_DE_ANDAMENTO, ARQUIVO_DE_PARADA};
use use_cases::recuperacao::ListarCartoesUseCase;

use super::elevar;
use super::estado::Recado;

pub trait Recuperador: Send + Sync + 'static {
    /// Responde **sempre** com `Recado::Cartoes`, mesmo vazio.
    fn cartoes(&self, canal: Sender<Recado>);
    /// Pede a senha e varre. Responde com `Andamento` enquanto lê, e termina
    /// **sempre** com `Terminou`, `Falhou` ou `Negado`.
    fn comecar(&self, cartao: CartaoBruto, destino: String, canal: Sender<Recado>);
    /// Pede à varredura em curso que pare no próximo bloco.
    fn parar(&self, destino: &str);
}

pub struct RecuperadorDoDisco {
    cartoes: Arc<dyn CartoesBrutos>,
}

impl RecuperadorDoDisco {
    pub fn novo(cartoes: Arc<dyn CartoesBrutos>) -> Self {
        Self { cartoes }
    }
}

impl Recuperador for RecuperadorDoDisco {
    fn cartoes(&self, canal: Sender<Recado>) {
        let cartoes = self.cartoes.clone();
        // Uma thread e não o tokio: no macOS e no Windows a lista chama
        // programas do sistema (`diskutil`, PowerShell), e é espera de processo,
        // não de rede.
        std::thread::spawn(move || {
            let lista = ListarCartoesUseCase::new(cartoes).execute();
            let _ = canal.send(Recado::Cartoes(lista));
        });
    }

    fn comecar(&self, cartao: CartaoBruto, destino: String, canal: Sender<Recado>) {
        std::thread::spawn(move || acompanhar(cartao, PathBuf::from(destino), canal));
    }

    fn parar(&self, destino: &str) {
        let _ = std::fs::write(Path::new(destino).join(ARQUIVO_DE_PARADA), b"");
    }
}

/// Roda o processo elevado e repassa o andamento que ele grava no destino.
fn acompanhar(cartao: CartaoBruto, destino: PathBuf, canal: Sender<Recado>) {
    if let Err(e) = std::fs::create_dir_all(&destino) {
        let _ = canal.send(Recado::Falhou(format!(
            "Não deu para criar a pasta {}: {e}",
            destino.display()
        )));
        return;
    }
    // O que sobrou de uma recuperação anterior na mesma pasta seria lido como
    // se fosse desta.
    let _ = std::fs::remove_file(destino.join(ARQUIVO_DE_ANDAMENTO));
    let _ = std::fs::remove_file(destino.join(ARQUIVO_DE_PARADA));

    let executavel = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            let _ = canal.send(Recado::Falhou(format!(
                "Não deu para achar o próprio programa: {e}"
            )));
            return;
        }
    };
    let mut comando = elevar::comando(
        &executavel,
        &cartao.dispositivo,
        &destino.to_string_lossy(),
        cartao.tamanho,
    );
    comando
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut filho = match comando.spawn() {
        Ok(f) => f,
        Err(e) => {
            let _ = canal.send(Recado::Falhou(format!(
                "Não deu para pedir a senha de administrador ({e}). No Linux, a \
                 recuperação precisa do pkexec (pacote polkit)."
            )));
            return;
        }
    };

    loop {
        std::thread::sleep(Duration::from_millis(300));
        if let Some(a) = ler_andamento(&destino) {
            let _ = canal.send(Recado::Andamento {
                lidos: a.lidos,
                total: a.total,
                achadas: a.achadas,
            });
        }
        match filho.try_wait() {
            Ok(None) => continue,
            Ok(Some(_)) => break,
            Err(e) => {
                let _ = canal.send(Recado::Falhou(format!(
                    "A recuperação saiu do controle: {e}"
                )));
                return;
            }
        }
    }
    let _ = canal.send(desfecho(ler_andamento(&destino)));
}

/// O que dizer à tela quando o processo elevado termina.
///
/// 🔑 **Sem andamento gravado, a varredura não começou**: o operador fechou o
/// pedido de senha, errou a senha, ou o UAC foi recusado. Os três sistemas
/// dizem isso com códigos de saída diferentes, e o arquivo é igual nos três.
pub fn desfecho(andamento: Option<infrastructure::recuperacao::AndamentoGravado>) -> Recado {
    match andamento {
        None => Recado::Negado,
        Some(a) => match a.erro {
            Some(erro) => Recado::Falhou(erro),
            None if a.terminou => Recado::Terminou {
                achadas: a.achadas,
                interrompida: a.interrompida,
                ilegiveis: a.ilegiveis,
            },
            None => Recado::Falhou(
                "A recuperação parou no meio sem dizer por quê. As fotos gravadas até \
                 ali estão na pasta."
                    .into(),
            ),
        },
    }
}

/// Um recuperador que responde o que o teste mandar.
#[cfg(test)]
pub mod mentira {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    pub struct RecuperadorDeMentira {
        pub lista: Mutex<Vec<CartaoBruto>>,
        /// O que a varredura responde, na ordem.
        pub respostas: Mutex<Vec<Recado>>,
        pub comecados: Mutex<Vec<(String, String)>>,
        pub paradas: Mutex<Vec<String>>,
    }

    impl Recuperador for RecuperadorDeMentira {
        fn cartoes(&self, canal: Sender<Recado>) {
            let _ = canal.send(Recado::Cartoes(self.lista.lock().unwrap().clone()));
        }

        fn comecar(&self, cartao: CartaoBruto, destino: String, canal: Sender<Recado>) {
            self.comecados
                .lock()
                .unwrap()
                .push((cartao.dispositivo, destino));
            for r in self.respostas.lock().unwrap().drain(..) {
                let _ = canal.send(r);
            }
        }

        fn parar(&self, destino: &str) {
            self.paradas.lock().unwrap().push(destino.to_string());
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use infrastructure::recuperacao::AndamentoGravado;

    #[test]
    fn sem_andamento_e_senha_negada() {
        assert_eq!(desfecho(None), Recado::Negado);
    }

    #[test]
    fn o_erro_gravado_chega_como_falha() {
        let a = AndamentoGravado {
            terminou: true,
            erro: Some("O cartão saiu do leitor?".into()),
            ..Default::default()
        };
        assert_eq!(
            desfecho(Some(a)),
            Recado::Falhou("O cartão saiu do leitor?".into())
        );
    }

    #[test]
    fn o_fim_gravado_chega_com_os_numeros() {
        let a = AndamentoGravado {
            terminou: true,
            achadas: 12,
            interrompida: true,
            ..Default::default()
        };
        assert_eq!(
            desfecho(Some(a)),
            Recado::Terminou {
                achadas: 12,
                interrompida: true,
                ilegiveis: 0
            }
        );
    }
}
