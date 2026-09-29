//! O painel de recuperação, desenhado dentro do modal de Importação.
//!
//! Três passos numa tela só: **qual cartão**, **para onde** e **recuperar**. No
//! fim, "Importar as N fotos" devolve o operador à grade da importação,
//! apontada para a pasta onde as fotos voltaram.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Sizable};
use gpui_kit::{div, prelude::*, px, Context, EventEmitter, SharedString, Task, Window};

use super::estado::{aplicar, frase, Estado, Fase, Recado};
use super::porta::Recuperador;
use crate::importacao::estado::Recado as RecadoDaImportacao;
use crate::importacao::explorador::SeletorDePasta;

const INTERVALO_DE_COLHEITA: Duration = Duration::from_millis(150);

/// O que o painel pede ao modal que o contém.
#[derive(Debug, Clone, PartialEq)]
pub enum Pedido {
    /// Importar o que voltou, desta pasta.
    Importar(String),
    /// Voltar à importação sem nada.
    Fechar,
}

impl EventEmitter<Pedido> for Recuperacao {}

pub struct Recuperacao {
    pub estado: Estado,
    porta: Arc<dyn Recuperador>,
    seletor: Arc<dyn SeletorDePasta>,
    recados: (Sender<Recado>, Receiver<Recado>),
    /// O seletor de pasta é o da importação e responde no recado dela.
    do_seletor: (Sender<RecadoDaImportacao>, Receiver<RecadoDaImportacao>),
    esperando_seletor: bool,
    esperando_cartoes: bool,
    colhendo: bool,
    _colheita: Option<Task<()>>,
}

impl Recuperacao {
    pub fn nova(porta: Arc<dyn Recuperador>, seletor: Arc<dyn SeletorDePasta>) -> Self {
        Self {
            estado: Estado::default(),
            porta,
            seletor,
            recados: channel(),
            do_seletor: channel(),
            esperando_seletor: false,
            esperando_cartoes: false,
            colhendo: false,
            _colheita: None,
        }
    }

    /// Pede a lista de cartões. Chamada a cada vez que o painel aparece: o
    /// cartão costuma ser plugado **depois** de o operador perceber o engano.
    pub fn listar(&mut self, cx: &mut Context<Self>) {
        if self.estado.recuperando() {
            return;
        }
        self.esperando_cartoes = true;
        self.porta.cartoes(self.recados.0.clone());
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn escolher_destino(&mut self, cx: &mut Context<Self>) {
        if self.estado.recuperando() {
            return;
        }
        self.esperando_seletor = true;
        self.seletor.escolher_destino(self.do_seletor.0.clone(), cx);
        self.acompanhar(cx);
        cx.notify();
    }

    pub fn comecar(&mut self, cx: &mut Context<Self>) {
        if let Some((cartao, destino)) = self.estado.comecar() {
            self.porta.comecar(cartao, destino, self.recados.0.clone());
            self.acompanhar(cx);
        }
        cx.notify();
    }

    pub fn parar(&mut self, cx: &mut Context<Self>) {
        if self.estado.pedir_parada() {
            if let Some(destino) = self.estado.destino.as_deref() {
                self.porta.parar(destino);
            }
        }
        cx.notify();
    }

    fn acompanhar(&mut self, cx: &mut Context<Self>) {
        if self.colhendo {
            return;
        }
        self.colhendo = true;
        self._colheita = Some(cx.spawn(async move |esta, cx| loop {
            cx.background_executor().timer(INTERVALO_DE_COLHEITA).await;
            let Ok(continua) = esta.update(cx, |tela, cx| tela.colher(cx)) else {
                break;
            };
            if !continua {
                break;
            }
        }));
    }

    /// Drena os dois canais. Devolve se vale continuar acordando.
    pub fn colher(&mut self, cx: &mut Context<Self>) -> bool {
        let mut mudou = false;
        while let Ok(recado) = self.do_seletor.1.try_recv() {
            mudou = true;
            self.esperando_seletor = false;
            let destino = match recado {
                RecadoDaImportacao::DestinoEscolhido(p) => Some(p),
                _ => None,
            };
            aplicar(&mut self.estado, Recado::Destino(destino));
        }
        while let Ok(recado) = self.recados.1.try_recv() {
            mudou = true;
            if matches!(recado, Recado::Cartoes(_)) {
                self.esperando_cartoes = false;
            }
            aplicar(&mut self.estado, recado);
        }
        if mudou {
            cx.notify();
        }
        let continua =
            self.esperando_seletor || self.esperando_cartoes || self.estado.recuperando();
        if !continua {
            self.colhendo = false;
        }
        continua
    }

    fn lista_de_cartoes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let travado = self.estado.recuperando();
        let apagado = cx.theme().muted_foreground;
        let mut lista = div().flex().flex_col().gap(px(4.));
        if !self.estado.listou {
            lista = lista.child(
                div()
                    .text_xs()
                    .text_color(apagado)
                    .child("Procurando cartões…"),
            );
        } else if self.estado.cartoes.is_empty() {
            lista = lista.child(div().text_xs().text_color(apagado).child(
                "Nenhum cartão plugado. Coloque o cartão no leitor e clique em Procurar de novo.",
            ));
        }
        for (i, cartao) in self.estado.cartoes.iter().enumerate() {
            let escolhido = self.estado.escolhido == Some(i);
            let rotulo = format!(
                "{} · {} · {}",
                cartao.nome,
                tamanho_legivel(cartao.tamanho),
                cartao.dispositivo
            );
            lista = lista.child(
                Button::new(SharedString::from(format!("cartao-{i}")))
                    .label(SharedString::from(rotulo))
                    .small()
                    .when(escolhido, |b| b.primary())
                    .selected(escolhido)
                    .disabled(travado)
                    .on_click(cx.listener(move |tela, _ev, _window, cx| {
                        tela.estado.escolher_cartao(i);
                        cx.notify();
                    })),
            );
        }
        lista.child(
            div().child(
                Button::new("procurar-cartoes")
                    .label("Procurar de novo")
                    .xsmall()
                    .disabled(travado)
                    .on_click(cx.listener(|tela, _ev, _window, cx| tela.listar(cx))),
            ),
        )
    }

    fn andamento(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let texto = frase(&self.estado.fase);
        if texto.is_empty() {
            return None;
        }
        let valor = match self.estado.fase {
            Fase::Recuperando { lidos, total, .. } if total > 0 => {
                (lidos as f32 / total as f32 * 100.0).min(100.0)
            }
            Fase::Terminou { .. } => 100.0,
            _ => 0.0,
        };
        Some(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    Progress::new("recuperacao-andamento")
                        .h(px(6.))
                        .value(valor)
                        .color(cx.theme().primary),
                )
                .child(div().text_sm().child(SharedString::from(texto))),
        )
    }

    fn botoes(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut linha = div().flex().items_center().gap(px(8.)).pt(px(8.));
        match &self.estado.fase {
            Fase::Escolhendo => {
                linha = linha
                    .child(
                        Button::new("recuperacao-voltar")
                            .label("Voltar à importação")
                            .xsmall()
                            .on_click(cx.listener(|_tela, _ev, _window, cx| {
                                cx.emit(Pedido::Fechar);
                            })),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("recuperacao-comecar")
                            .label("Recuperar")
                            .xsmall()
                            .primary()
                            .disabled(!self.estado.pode_comecar())
                            .on_click(cx.listener(|tela, _ev, _window, cx| tela.comecar(cx))),
                    );
            }
            Fase::Recuperando { parando, .. } => {
                linha = linha.child(div().flex_1()).child(
                    Button::new("recuperacao-parar")
                        .label("Parar")
                        .xsmall()
                        .danger()
                        .disabled(*parando)
                        .on_click(cx.listener(|tela, _ev, _window, cx| tela.parar(cx))),
                );
            }
            Fase::Terminou { achadas, .. } => {
                let achadas = *achadas;
                linha = linha
                    .child(
                        Button::new("recuperacao-outro")
                            .label("Recuperar outro cartão")
                            .xsmall()
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.estado.recomecar();
                                tela.listar(cx);
                            })),
                    )
                    .child(div().flex_1())
                    .when(achadas == 0, |l| {
                        l.child(
                            Button::new("recuperacao-voltar-fim")
                                .label("Voltar à importação")
                                .xsmall()
                                .on_click(cx.listener(|_tela, _ev, _window, cx| {
                                    cx.emit(Pedido::Fechar);
                                })),
                        )
                    })
                    .when(achadas > 0, |l| {
                        l.child(
                            Button::new("recuperacao-importar")
                                .label(SharedString::from(format!(
                                    "Importar {achadas} {}",
                                    if achadas == 1 { "foto" } else { "fotos" }
                                )))
                                .xsmall()
                                .primary()
                                .on_click(cx.listener(|tela, _ev, _window, cx| {
                                    if let Some(destino) = tela.estado.destino.clone() {
                                        cx.emit(Pedido::Importar(destino));
                                    }
                                })),
                        )
                    });
            }
        }
        linha
    }
}

impl Render for Recuperacao {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let apagado = cx.theme().muted_foreground;
        let destino: SharedString = self
            .estado
            .destino
            .clone()
            .unwrap_or_else(|| "Nenhuma pasta escolhida".into())
            .into();
        let travado = self.estado.recuperando();

        div()
            .id("recuperacao")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .gap(px(12.))
            .py(px(8.))
            .child(
                div()
                    .text_base()
                    .child("Recuperar fotos de um cartão formatado"),
            )
            .child(div().text_xs().text_color(cx.theme().warning).child(
                "🚨 Não tire fotos nem grave nada neste cartão até terminar: cada \
                         arquivo novo pode apagar de vez uma foto que ainda dá para salvar.",
            ))
            .child(div().text_sm().child("1. O cartão"))
            .child(self.lista_de_cartoes(cx))
            .child(
                div()
                    .text_sm()
                    .child("2. Onde guardar as fotos recuperadas"),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        Button::new("recuperacao-destino")
                            .label("Escolher pasta…")
                            .xsmall()
                            .disabled(travado)
                            .on_click(cx.listener(|tela, _ev, _window, cx| {
                                tela.escolher_destino(cx);
                            })),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .truncate()
                            .text_color(apagado)
                            .child(destino),
                    ),
            )
            .child(div().text_xs().text_color(apagado).child(
                "3. Recuperar. O sistema vai pedir a senha de administrador: ler o \
                         cartão setor por setor exige isso. Fotos que ficaram em pedaços no \
                         cartão não voltam; as que voltam, voltam inteiras.",
            ))
            .when_some(self.estado.aviso.clone(), |tela, aviso| {
                tela.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(SharedString::from(aviso)),
                )
            })
            .children(self.andamento(cx))
            .child(self.botoes(cx))
    }
}

fn tamanho_legivel(bytes: u64) -> String {
    const GB: f64 = 1_000_000_000.0;
    const MB: f64 = 1_000_000.0;
    let b = bytes as f64;
    if b >= GB {
        format!("{:.1} GB", b / GB)
    } else {
        format!("{:.0} MB", b / MB)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::importacao::explorador::mentira::SeletorDeMentira;
    use crate::recuperacao::porta::mentira::RecuperadorDeMentira;
    use domain::recuperacao::CartaoBruto;
    use gpui_kit::TestAppContext;

    fn cartao() -> CartaoBruto {
        CartaoBruto {
            dispositivo: "/dev/mmcblk0".into(),
            nome: "EOS_DIGITAL".into(),
            tamanho: 64_000_000_000,
            montagens: vec!["/media/ana/EOS_DIGITAL".into()],
        }
    }

    fn janela(
        cx: &mut TestAppContext,
        porta: Arc<RecuperadorDeMentira>,
        destino: &str,
    ) -> gpui_kit::WindowHandle<Recuperacao> {
        cx.update(gpui_kit::init);
        let seletor = Arc::new(SeletorDeMentira::escolhe(destino));
        cx.add_window(move |_window, _cx| Recuperacao::nova(porta, seletor))
    }

    fn colher(cx: &mut TestAppContext, j: &gpui_kit::WindowHandle<Recuperacao>) {
        for _ in 0..5 {
            let _ = j.update(cx, |t, _w, cx| t.colher(cx));
            cx.run_until_parked();
        }
    }

    #[gpui_kit::test]
    fn do_cartao_ao_pedido_de_importar(cx: &mut TestAppContext) {
        let porta = Arc::new(RecuperadorDeMentira::default());
        *porta.lista.lock().unwrap() = vec![cartao()];
        *porta.respostas.lock().unwrap() = vec![
            Recado::Andamento {
                lidos: 32_000_000_000,
                total: 64_000_000_000,
                achadas: 40,
            },
            Recado::Terminou {
                achadas: 83,
                interrompida: false,
                ilegiveis: 0,
            },
        ];
        let j = janela(cx, porta.clone(), "/home/ana/Recuperadas");
        let pedidos = Arc::new(std::sync::Mutex::new(Vec::new()));
        {
            let pedidos = pedidos.clone();
            let entidade = j.root(cx).unwrap();
            cx.update(|cx| {
                cx.subscribe(&entidade, move |_e, pedido: &Pedido, _cx| {
                    pedidos.lock().unwrap().push(pedido.clone());
                })
                .detach();
            });
        }

        j.update(cx, |t, _w, cx| t.listar(cx)).unwrap();
        colher(cx, &j);
        j.update(cx, |t, _w, cx| t.escolher_destino(cx)).unwrap();
        colher(cx, &j);
        j.update(cx, |t, _w, cx| {
            assert_eq!(t.estado.escolhido, Some(0));
            assert!(t.estado.pode_comecar());
            t.comecar(cx);
        })
        .unwrap();
        colher(cx, &j);

        assert_eq!(
            *porta.comecados.lock().unwrap(),
            [(
                "/dev/mmcblk0".to_string(),
                "/home/ana/Recuperadas".to_string()
            )]
        );
        j.update(cx, |t, _w, cx| {
            assert_eq!(t.estado.recuperadas(), Some(83));
            cx.emit(Pedido::Importar(t.estado.destino.clone().unwrap()));
        })
        .unwrap();
        cx.run_until_parked();
        assert_eq!(
            *pedidos.lock().unwrap(),
            [Pedido::Importar("/home/ana/Recuperadas".into())]
        );
    }

    #[gpui_kit::test]
    fn destino_no_cartao_nao_comeca(cx: &mut TestAppContext) {
        let porta = Arc::new(RecuperadorDeMentira::default());
        *porta.lista.lock().unwrap() = vec![cartao()];
        let j = janela(cx, porta.clone(), "/media/ana/EOS_DIGITAL/salvar");
        j.update(cx, |t, _w, cx| t.listar(cx)).unwrap();
        colher(cx, &j);
        j.update(cx, |t, _w, cx| t.escolher_destino(cx)).unwrap();
        colher(cx, &j);
        j.update(cx, |t, _w, cx| t.comecar(cx)).unwrap();
        assert!(porta.comecados.lock().unwrap().is_empty());
        j.update(cx, |t, _w, _cx| {
            assert!(t
                .estado
                .aviso
                .as_deref()
                .unwrap()
                .contains("próprio cartão"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn parar_chega_a_porta(cx: &mut TestAppContext) {
        let porta = Arc::new(RecuperadorDeMentira::default());
        *porta.lista.lock().unwrap() = vec![cartao()];
        let j = janela(cx, porta.clone(), "/home/ana/Rec");
        j.update(cx, |t, _w, cx| t.listar(cx)).unwrap();
        colher(cx, &j);
        j.update(cx, |t, _w, cx| t.escolher_destino(cx)).unwrap();
        colher(cx, &j);
        j.update(cx, |t, _w, cx| {
            t.comecar(cx);
            t.parar(cx);
            t.parar(cx);
        })
        .unwrap();
        assert_eq!(*porta.paradas.lock().unwrap(), ["/home/ana/Rec"]);
    }
}
