//! ⏏️ O cartão sai sozinho ao fim de cada importação.
//!
//! 🚨 **Pedido do dono, 24/out/2026**: no Linux, com a câmera em PTP, o cartão
//! arrancado sem ejetar comprometia a importação seguinte (a montagem do GVfs
//! ficava pendurada). Agora o app ejeta o cartão de onde o lote veio e diz ao
//! operador, sem meias palavras, se já pode tirar — ou que **não** pode.
//!
//! 🔑 **Um embrulho do [`Importador`], e não um passo em cada tela.** Três telas
//! importam do cartão (o modal da Biblioteca, o assistente de nova sessão e o
//! "Importar fotos" da sessão aberta), e todas passam pela mesma porta. O
//! embrulho vê o lote terminar e decide; quem avisa é a raiz do app, por cima de
//! qualquer tela ([`Ejecoes`]).
//!
//! ⚠️ **Espera a fila esvaziar.** As telas da sessão guardam levas na fila
//! enquanto uma copia, e a próxima sai logo depois do fim da anterior. Ejetar no
//! fim da primeira faria a segunda falhar foto a foto. Por isso o fim de um lote
//! só ejeta depois de uma folga sem lote novo no ar — e quem ejeta é o último.
//! Lote cancelado não ejeta: quem cancela costuma refazer a escolha no mesmo
//! cartão.

use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use domain::value_objects::ImportOptions;

use super::explorador::{Andamento, Freios, Importador};

/// Quanto o fim de um lote espera por outro antes de ejetar.
pub const FOLGA: Duration = Duration::from_secs(4);

/// De onde o lote veio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CartaoDoLote {
    pub nome: String,
    pub raiz: String,
}

/// O que a raiz do app conta ao operador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ejecao {
    /// 🚨 A cópia acabou e a ejeção começou: **ainda não tire**. Com o cartão
    /// preso as tentativas levam até ~20 s, e sem este aviso a tela ficaria
    /// muda justo quando o balconista estende a mão para o leitor.
    Ejetando { cartao: CartaoDoLote },
    /// Saiu: pode tirar.
    Ejetado { cartao: CartaoDoLote },
    /// Não saiu: **não** tire.
    NaoEjetado {
        cartao: CartaoDoLote,
        motivo: String,
    },
}

/// Quem sabe achar o cartão de um arquivo e ejetá-lo.
pub trait Ejetor: Send + Sync + 'static {
    /// `None` quando o arquivo está numa pasta do disco do sistema.
    fn cartao_de(&self, arquivo: &str) -> Option<CartaoDoLote>;
    /// Bloqueante: roda numa thread de fundo.
    fn ejetar(&self, cartao: &CartaoDoLote) -> Result<(), String>;
}

/// O ejetor de verdade — `infrastructure::devices::ejetar`.
pub struct EjetorDoSistema;

impl Ejetor for EjetorDoSistema {
    fn cartao_de(&self, arquivo: &str) -> Option<CartaoDoLote> {
        infrastructure::devices::ejetar::cartao_de(std::path::Path::new(arquivo)).map(|cartao| {
            CartaoDoLote {
                nome: cartao.nome,
                raiz: cartao.raiz.to_string_lossy().into_owned(),
            }
        })
    }

    fn ejetar(&self, cartao: &CartaoDoLote) -> Result<(), String> {
        let raiz = std::path::Path::new(&cartao.raiz);
        // Já saiu (o operador ejetou pelo Finder, ou tirou): nada a fazer.
        let Some(montado) = infrastructure::devices::ejetar::cartoes_montados()
            .into_iter()
            .find(|c| c.raiz == raiz)
        else {
            return Ok(());
        };
        infrastructure::devices::ejetar::ejetar(&montado)
    }
}

/// O lado da raiz: os avisos que chegam e o "Tentar de novo".
#[derive(Clone)]
pub struct Ejecoes {
    avisos: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedReceiver<Ejecao>>>>,
    remetente: tokio::sync::mpsc::UnboundedSender<Ejecao>,
    ejetor: Arc<dyn Ejetor>,
}

impl Ejecoes {
    /// Sem ejeção nenhuma — os testes e as montagens sem cartão.
    pub fn desligada() -> Self {
        struct Nenhum;
        impl Ejetor for Nenhum {
            fn cartao_de(&self, _: &str) -> Option<CartaoDoLote> {
                None
            }
            fn ejetar(&self, _: &CartaoDoLote) -> Result<(), String> {
                Ok(())
            }
        }
        let (remetente, avisos) = tokio::sync::mpsc::unbounded_channel();
        Self {
            avisos: Arc::new(Mutex::new(Some(avisos))),
            remetente,
            ejetor: Arc::new(Nenhum),
        }
    }

    /// Os avisos, para quem vai mostrá-los. Só a primeira chamada os leva.
    pub fn tomar_avisos(&self) -> Option<tokio::sync::mpsc::UnboundedReceiver<Ejecao>> {
        self.avisos.lock().ok()?.take()
    }

    /// O operador fechou o que prendia o cartão e pediu de novo.
    pub fn tentar_de_novo(&self, cartao: CartaoDoLote) {
        let ejetor = self.ejetor.clone();
        let remetente = self.remetente.clone();
        std::thread::spawn(move || {
            let _ = remetente.send(Ejecao::Ejetando {
                cartao: cartao.clone(),
            });
            let _ = remetente.send(ejetar_e_contar(ejetor.as_ref(), cartao));
        });
    }
}

fn ejetar_e_contar(ejetor: &dyn Ejetor, cartao: CartaoDoLote) -> Ejecao {
    match ejetor.ejetar(&cartao) {
        Ok(()) => Ejecao::Ejetado { cartao },
        Err(motivo) => Ejecao::NaoEjetado { cartao, motivo },
    }
}

#[derive(Default)]
struct Pendentes {
    /// Lotes copiando agora.
    no_ar: usize,
    /// Sobe a cada lote que começa: é como o fim de um lote sabe, depois da
    /// folga, se outro começou nesse meio tempo.
    geracao: u64,
    /// Os cartões dos lotes que terminaram, esperando a fila esvaziar.
    a_ejetar: Vec<CartaoDoLote>,
}

/// O importador que ejeta o cartão quando o último lote dele termina.
pub struct ImportadorQueEjeta {
    de_dentro: Arc<dyn Importador>,
    ejetor: Arc<dyn Ejetor>,
    pendentes: Arc<Mutex<Pendentes>>,
    avisos: tokio::sync::mpsc::UnboundedSender<Ejecao>,
    folga: Duration,
}

impl ImportadorQueEjeta {
    pub fn novo(
        de_dentro: Arc<dyn Importador>,
        ejetor: Arc<dyn Ejetor>,
        folga: Duration,
    ) -> (Self, Ejecoes) {
        let (remetente, avisos) = tokio::sync::mpsc::unbounded_channel();
        let ejecoes = Ejecoes {
            avisos: Arc::new(Mutex::new(Some(avisos))),
            remetente: remetente.clone(),
            ejetor: ejetor.clone(),
        };
        (
            Self {
                de_dentro,
                ejetor,
                pendentes: Arc::default(),
                avisos: remetente,
                folga,
            },
            ejecoes,
        )
    }
}

impl Importador for ImportadorQueEjeta {
    fn importar(
        &self,
        arquivos: Vec<String>,
        opcoes: ImportOptions,
        freios: Freios,
        canal: Sender<Andamento>,
    ) {
        if let Ok(mut pendentes) = self.pendentes.lock() {
            pendentes.no_ar += 1;
            pendentes.geracao += 1;
        }
        let primeiro = arquivos.first().cloned();
        let (repasse, recebe) = channel();
        self.de_dentro
            .importar(arquivos, opcoes, freios.clone(), repasse);

        let ejetor = self.ejetor.clone();
        let pendentes = self.pendentes.clone();
        let avisos = self.avisos.clone();
        let folga = self.folga;
        std::thread::spawn(move || {
            // 🔑 Aqui, e não na thread da tela: achar o cartão relê a lista de
            // montagens do sistema, e um disco de rede morto a prenderia.
            let cartao = primeiro.and_then(|arquivo| ejetor.cartao_de(&arquivo));
            // O repasse acaba no `Terminou`, ou quando o importador solta o
            // canal sem dizer (a tela fechou, o lote morreu).
            while let Ok(andamento) = recebe.recv() {
                let fim = matches!(andamento, Andamento::Terminou { .. });
                let _ = canal.send(andamento);
                if fim {
                    break;
                }
            }

            let geracao = {
                let Ok(mut p) = pendentes.lock() else {
                    return;
                };
                p.no_ar = p.no_ar.saturating_sub(1);
                if let Some(cartao) = cartao {
                    if freios.cancelada() {
                        p.a_ejetar.retain(|c| c.raiz != cartao.raiz);
                    } else if !p.a_ejetar.iter().any(|c| c.raiz == cartao.raiz) {
                        p.a_ejetar.push(cartao);
                    }
                }
                p.geracao
            };

            std::thread::sleep(folga);
            let prontos = {
                let Ok(mut p) = pendentes.lock() else {
                    return;
                };
                if p.no_ar > 0 || p.geracao != geracao {
                    // Outro lote está no ar: o fim dele decide.
                    return;
                }
                std::mem::take(&mut p.a_ejetar)
            };
            for cartao in prontos {
                let _ = avisos.send(Ejecao::Ejetando {
                    cartao: cartao.clone(),
                });
                let _ = avisos.send(ejetar_e_contar(ejetor.as_ref(), cartao));
            }
        });
    }

    fn reveladas_fora(&self) -> Vec<String> {
        self.de_dentro.reveladas_fora()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::importacao::explorador::mentira::ImportadorDeMentira;

    #[derive(Default)]
    struct EjetorDeMentira {
        ejetados: Mutex<Vec<String>>,
        recusar: Option<String>,
    }

    impl Ejetor for EjetorDeMentira {
        fn cartao_de(&self, arquivo: &str) -> Option<CartaoDoLote> {
            arquivo.starts_with("/Volumes/EOS/").then(|| CartaoDoLote {
                nome: "EOS".into(),
                raiz: "/Volumes/EOS".into(),
            })
        }

        fn ejetar(&self, cartao: &CartaoDoLote) -> Result<(), String> {
            self.ejetados.lock().unwrap().push(cartao.raiz.clone());
            match &self.recusar {
                Some(motivo) => Err(motivo.clone()),
                None => Ok(()),
            }
        }
    }

    const FOLGA_DO_TESTE: Duration = Duration::from_millis(150);

    fn montar(
        ejetor: Arc<EjetorDeMentira>,
    ) -> (
        ImportadorQueEjeta,
        tokio::sync::mpsc::UnboundedReceiver<Ejecao>,
    ) {
        let (importador, ejecoes) = ImportadorQueEjeta::novo(
            Arc::new(ImportadorDeMentira::default()),
            ejetor,
            FOLGA_DO_TESTE,
        );
        (importador, ejecoes.tomar_avisos().unwrap())
    }

    fn importar(importador: &ImportadorQueEjeta, arquivos: &[&str], freios: Freios) -> usize {
        let (canal, recebe) = channel();
        importador.importar(
            arquivos.iter().map(|a| a.to_string()).collect(),
            ImportOptions::default(),
            freios,
            canal,
        );
        // A tela recebe tudo, com o `Terminou` no fim.
        let mut contados = 0;
        while let Ok(andamento) = recebe.recv_timeout(Duration::from_secs(2)) {
            contados += 1;
            if matches!(andamento, Andamento::Terminou { .. }) {
                break;
            }
        }
        contados
    }

    /// O resultado da próxima ejeção — conferindo que o "Ejetando" veio antes.
    fn proximo(avisos: &mut tokio::sync::mpsc::UnboundedReceiver<Ejecao>) -> Option<Ejecao> {
        std::thread::sleep(FOLGA_DO_TESTE * 3);
        let primeiro = avisos.try_recv().ok()?;
        assert!(
            matches!(primeiro, Ejecao::Ejetando { .. }),
            "o aviso de não tirar vem antes do resultado: {primeiro:?}"
        );
        avisos.try_recv().ok()
    }

    #[test]
    fn o_lote_do_cartao_ejeta_e_avisa() {
        let ejetor = Arc::new(EjetorDeMentira::default());
        let (importador, mut avisos) = montar(ejetor.clone());

        let contados = importar(
            &importador,
            &["/Volumes/EOS/DCIM/a.CR3", "/Volumes/EOS/DCIM/b.CR3"],
            Freios::default(),
        );

        assert_eq!(contados, 4, "Comecou + 2 Feito + Terminou chegam à tela");
        assert_eq!(
            proximo(&mut avisos),
            Some(Ejecao::Ejetado {
                cartao: CartaoDoLote {
                    nome: "EOS".into(),
                    raiz: "/Volumes/EOS".into()
                }
            })
        );
        assert_eq!(*ejetor.ejetados.lock().unwrap(), vec!["/Volumes/EOS"]);
    }

    #[test]
    fn foto_de_pasta_nao_ejeta_nada() {
        let ejetor = Arc::new(EjetorDeMentira::default());
        let (importador, mut avisos) = montar(ejetor.clone());

        importar(&importador, &["/Users/alex/Fotos/a.jpg"], Freios::default());

        assert_eq!(proximo(&mut avisos), None);
        assert!(ejetor.ejetados.lock().unwrap().is_empty());
    }

    /// 🚨 Quem cancela costuma refazer a escolha no mesmo cartão.
    #[test]
    fn lote_cancelado_nao_ejeta() {
        let ejetor = Arc::new(EjetorDeMentira::default());
        let (importador, mut avisos) = montar(ejetor.clone());
        let freios = Freios::default();
        freios.cancelar();

        importar(&importador, &["/Volumes/EOS/DCIM/a.CR3"], freios);

        assert_eq!(proximo(&mut avisos), None);
    }

    /// 🚨 A leva da fila sai logo depois do fim da anterior: ejetar no meio a
    /// faria falhar foto a foto. Ejeta uma vez, depois da última.
    #[test]
    fn a_leva_da_fila_segura_a_ejecao_ate_o_fim() {
        let ejetor = Arc::new(EjetorDeMentira::default());
        let (importador, mut avisos) = montar(ejetor.clone());

        importar(&importador, &["/Volumes/EOS/DCIM/a.CR3"], Freios::default());
        importar(&importador, &["/Volumes/EOS/DCIM/b.CR3"], Freios::default());

        assert!(matches!(proximo(&mut avisos), Some(Ejecao::Ejetado { .. })));
        assert_eq!(proximo(&mut avisos), None, "uma ejeção só");
        assert_eq!(ejetor.ejetados.lock().unwrap().len(), 1);
    }

    #[test]
    fn cartao_preso_avisa_que_nao_pode_tirar() {
        let ejetor = Arc::new(EjetorDeMentira {
            recusar: Some("o cartão está em uso".into()),
            ..Default::default()
        });
        let (importador, mut avisos) = montar(ejetor);

        importar(&importador, &["/Volumes/EOS/DCIM/a.CR3"], Freios::default());

        assert_eq!(
            proximo(&mut avisos),
            Some(Ejecao::NaoEjetado {
                cartao: CartaoDoLote {
                    nome: "EOS".into(),
                    raiz: "/Volumes/EOS".into()
                },
                motivo: "o cartão está em uso".into(),
            })
        );
    }
}
