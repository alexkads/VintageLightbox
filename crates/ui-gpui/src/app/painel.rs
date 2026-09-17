//! A moldura do painel: o menu lateral, o cabeçalho, o menu da conta e o canto
//! dos envios. É o layout do dashboard do site, que o app Tauri mostra
//! (`frontend/desktop/src/app.tsx`), desenhado aqui com as mesmas medidas.
//!
//! # O que vem de onde no site
//!
//! | Peça | Site | Aqui |
//! |---|---|---|
//! | Menu lateral recolhível, começando recolhido | `MenuLateral` + `SidebarProvider defaultOpen={false}` | [`Aplicativo::menu_lateral`] |
//! | Só "Sessões fotográficas" e "Caixa", em "Operação" | `MENU_DO_APP` | [`MENU`] |
//! | Faixa de 56 px com o botão do menu e o assunto da página | `CabecalhoDoDashboard` + `NoCabecalho` | [`Aplicativo::cabecalho`] |
//! | A galeria não tem essa faixa: o botão do menu vai para a barra dela | `eAGaleriaDeUmaSessao` | `Detalhe`, pedido `AlternarMenu` |
//! | Conta, tema (Claro, Escuro, Sistema) e Sair | `MenuDoUsuario` | [`Aplicativo::menu_da_conta`] |
//! | "N envios na fila" e "N envios recusados", no canto | `FilaDeEnviosDoApp` | [`Aplicativo::canto_dos_envios`] |
//!
//! 🔑 **O que não está pronto no app não aparece nele** (DESKTOP_TAURI §0): o
//! menu tem as duas seções que as duas interfaces têm, e nenhuma a mais.

use gpui::{
    div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton, SharedString, Window,
};
use gpui_component::tooltip::Tooltip;
use gpui_component::{h_flex, v_flex, ActiveTheme, Icon};

use super::{Aplicativo, Tela};
use crate::estilo;
use crate::pos_venda::porta::{PedidoJson, Recado};
use crate::recursos::Icone;
use crate::tema::{self, Escolha};

/// `--sidebar-width` do shadcn: 16rem.
pub(super) const LARGURA_ABERTO: f32 = 256.;
/// `--sidebar-width-icon`: 3rem.
pub(super) const LARGURA_RECOLHIDO: f32 = 48.;
/// `h-14` do cabeçalho do dashboard.
pub(super) const ALTURA_DO_CABECALHO: f32 = 56.;

/// Quem está logado, lido de `/auth/me`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Conta {
    pub nome: Option<String>,
    pub email: String,
}

impl Conta {
    /// A conta na resposta de `/auth/me`, esteja ela no topo ou dentro de
    /// `user`/`data` (o mesmo leitor do app Tauri, `email_da_resposta`).
    pub fn da_resposta(valor: &serde_json::Value) -> Option<Conta> {
        let usuario = ["", "/user", "/data", "/data/user"]
            .iter()
            .filter_map(|p| valor.pointer(p))
            .find(|v| v.get("email").is_some())?;
        let email = usuario.get("email")?.as_str()?.to_string();
        let nome = ["nome", "name"]
            .iter()
            .find_map(|c| usuario.get(*c).and_then(|v| v.as_str()))
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string);
        Some(Conta { nome, email })
    }

    /// O nome, ou o e-mail quando não há nome (como o site).
    pub fn exibido(&self) -> &str {
        self.nome.as_deref().unwrap_or(&self.email)
    }

    /// A letra do retrato.
    pub fn inicial(&self) -> String {
        self.exibido()
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "U".into())
    }
}

/// Uma seção do menu lateral.
struct ItemDoMenu {
    tela: Tela,
    titulo: &'static str,
    icone: Icone,
}

/// As seções que o app tem, em "Operação" — as mesmas do app Tauri.
const MENU: [ItemDoMenu; 2] = [
    ItemDoMenu {
        tela: Tela::Sessoes,
        titulo: "Sessões fotográficas",
        icone: Icone::Camera,
    },
    ItemDoMenu {
        tela: Tela::Caixa,
        titulo: "Caixa",
        icone: Icone::Calculator,
    },
];

impl Tela {
    /// A seção do menu que fica acesa nesta tela. Tudo o que nasce na lista de
    /// sessões (a galeria, a revelação, a retenção) acende "Sessões".
    fn secao(self) -> Tela {
        match self {
            Tela::Caixa => Tela::Caixa,
            _ => Tela::Sessoes,
        }
    }

    /// Se a tela tem o menu lateral. A revelação cobre a janela inteira, como
    /// o editor do site (`fixed inset-0`).
    pub(super) fn tem_menu(self) -> bool {
        !matches!(self, Tela::Revelacao)
    }

    /// Se a tela tem a faixa do cabeçalho. A galeria tem a barra dela.
    pub(super) fn tem_cabecalho(self) -> bool {
        !matches!(self, Tela::Revelacao | Tela::Sessao | Tela::NovaSessao)
    }
}

impl Aplicativo {
    /// Troca de tela pelo menu, pelo cabeçalho ou pela barra de uma tela.
    ///
    /// 🔑 **Sair da galeria é sair da sessão**, como mudar de rota no site: a
    /// grade, a fila e as teclas de triagem deixam de valer para o ensaio que
    /// estava aberto. A revelação e a impressão são de dentro da galeria, e
    /// não passam por aqui.
    pub fn ir_para(&mut self, tela: Tela, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_da_conta = false;
        if tela != Tela::Sessao && self.sessao_aberta.is_some() {
            self.sair_da_sessao(cx);
        }
        match tela {
            Tela::Sessoes => self.sessoes.update(cx, |t, cx| t.recarregar(cx)),
            Tela::Caixa => self.caixa.update(cx, |t, cx| t.abrir(cx)),
            Tela::Retencao => self.retencao.update(cx, |t, cx| t.abrir(window, cx)),
            Tela::NovaSessao => self.nova_sessao.update(cx, |t, cx| t.abrir(window, cx)),
            _ => {}
        }
        self.tela = tela;
        window.focus(&self.foco);
        cx.notify();
    }

    pub fn menu_lateral_aberto(&self) -> bool {
        self.menu_aberto
    }

    /// O `Ctrl/Cmd+B` e o botão do cabeçalho.
    pub fn alternar_menu_lateral(&mut self, cx: &mut Context<Self>) {
        self.menu_aberto = !self.menu_aberto;
        cx.notify();
    }

    pub fn alternar_menu_da_conta(&mut self, cx: &mut Context<Self>) {
        self.menu_da_conta = !self.menu_da_conta;
        cx.notify();
    }

    pub fn menu_da_conta_aberto(&self) -> bool {
        self.menu_da_conta
    }

    pub fn escolha_de_tema(&self) -> Escolha {
        self.escolha_de_tema
    }

    /// Claro, Escuro ou Sistema — e a escolha fica para a próxima abertura.
    pub fn escolher_tema(&mut self, escolha: Escolha, window: &mut Window, cx: &mut Context<Self>) {
        self.escolha_de_tema = escolha;
        tema::guardar_escolha(&self.arquivo_do_tema, escolha);
        tema::aplicar(escolha, Some(window), cx);
        cx.notify();
    }

    /// O sistema trocou de claro para escuro (ou o contrário).
    pub(super) fn seguir_o_sistema(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.escolha_de_tema == Escolha::Sistema {
            tema::aplicar(Escolha::Sistema, Some(window), cx);
            cx.notify();
        }
    }

    /// O "Sair" do menu da conta: esquece a sessão e volta para a capa.
    pub fn sair_da_conta(&mut self, cx: &mut Context<Self>) {
        self.menu_da_conta = false;
        if self.sessao_aberta.is_some() {
            self.sair_da_sessao(cx);
        }
        let (canal, _) = std::sync::mpsc::channel();
        self.publicador.sair(canal);
        self.sessao = None;
        self.conta = None;
        self.tela = Tela::Sessoes;
        cx.notify();
    }

    pub fn conta(&self) -> Option<&Conta> {
        self.conta.as_ref()
    }

    /// Pede `/auth/me` para o menu da conta dizer quem está logado.
    pub(super) fn carregar_conta(&mut self, cx: &mut Context<Self>) {
        let Some(sessao) = self.sessao.clone() else {
            return;
        };
        self.publicador.pedir_json(
            sessao,
            PedidoJson::ler("conta", "/auth/me"),
            self.recados_da_conta.0.clone(),
        );
        self._conta = Some(cx.spawn(async move |raiz, cx| {
            for _ in 0..300 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;
                let Ok(chegou) = raiz.update(cx, |raiz, cx| raiz.colher_conta(cx)) else {
                    return;
                };
                if chegou {
                    return;
                }
            }
        }));
    }

    pub(crate) fn colher_conta(&mut self, cx: &mut Context<Self>) -> bool {
        let mut chegou = false;
        while let Ok(recado) = self.recados_da_conta.1.try_recv() {
            if let Recado::Json {
                rotulo: "conta",
                resultado,
            } = recado
            {
                chegou = true;
                match resultado {
                    Ok(valor) => self.conta = Conta::da_resposta(&valor),
                    Err(erro) => eprintln!("⚠️ [Conta] /auth/me: {erro}"),
                }
                cx.notify();
            }
        }
        chegou
    }

    // ── O desenho ────────────────────────────────────────────────────────

    pub(super) fn largura_do_menu(&self) -> f32 {
        if self.menu_aberto {
            LARGURA_ABERTO
        } else {
            LARGURA_RECOLHIDO
        }
    }

    /// O menu lateral, aberto (256 px) ou recolhido em ícones (48 px).
    pub(super) fn menu_lateral(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let aberto = self.menu_aberto;
        let tema = cx.theme();
        let (fundo, borda, texto, acento, apagado) = (
            tema.sidebar,
            tema.sidebar_border,
            tema.sidebar_foreground,
            tema.sidebar_accent,
            tema.sidebar_foreground.opacity(0.7),
        );
        let secao = self.tela.secao();
        let marca = self.marca(cx);
        let conta = self.botao_da_conta(cx);

        let itens: Vec<AnyElement> = MENU
            .iter()
            .map(|item| {
                let ativo = item.tela == secao;
                let destino = item.tela;
                let titulo = item.titulo;
                let base = h_flex()
                    .id(SharedString::from(format!("menu-{titulo}")))
                    .h(px(32.))
                    .rounded(px(8.))
                    .gap(px(8.))
                    .text_sm()
                    .cursor_pointer()
                    .hover(|s| s.bg(acento))
                    .when(ativo, |d| d.bg(acento).font_weight(FontWeight::MEDIUM))
                    .on_click(cx.listener(move |raiz, _, window, cx| {
                        raiz.ir_para(destino, window, cx);
                    }))
                    .child(Icon::new(item.icone).size(px(16.)));
                if aberto {
                    base.px(px(8.)).child(titulo).into_any_element()
                } else {
                    base.w(px(32.))
                        .justify_center()
                        .tooltip(move |window, cx| Tooltip::new(titulo).build(window, cx))
                        .into_any_element()
                }
            })
            .collect();

        v_flex()
            .id("menu-lateral")
            .flex_none()
            .h_full()
            .w(px(self.largura_do_menu()))
            .bg(fundo)
            .border_r_1()
            .border_color(borda)
            .text_color(texto)
            .overflow_hidden()
            // ── A marca ───────────────────────────────────────────────────
            .child(div().p(px(8.)).child(marca))
            // ── As seções ─────────────────────────────────────────────────
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .p(px(8.))
                    .when(aberto, |d| {
                        d.child(
                            h_flex()
                                .h(px(32.))
                                .px(px(8.))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(apagado)
                                .child("Operação"),
                        )
                    })
                    .children(itens),
            )
            .child(div().h(px(1.)).bg(borda))
            // ── A conta ───────────────────────────────────────────────────
            .child(div().p(px(8.)).child(conta))
    }

    fn marca(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let quadrado = div()
            .flex_none()
            .size(px(32.))
            .rounded(px(10.))
            .flex()
            .items_center()
            .justify_center()
            .bg(tema.sidebar_primary)
            .text_color(tema.sidebar_primary_foreground)
            .child(Icon::new(Icone::Camera).size(px(16.)));
        let acento = tema.sidebar_accent;
        let apagado = tema.sidebar_foreground.opacity(0.7);
        let base = h_flex()
            .id("menu-marca")
            .rounded(px(8.))
            .gap(px(8.))
            .cursor_pointer()
            .hover(|s| s.bg(acento))
            .on_click(cx.listener(|raiz, _, window, cx| {
                raiz.ir_para(Tela::Sessoes, window, cx);
            }))
            .child(quadrado);
        if self.menu_aberto {
            base.h(px(48.))
                .p(px(8.))
                .child(
                    v_flex()
                        .min_w(px(0.))
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .truncate()
                                .child("RecordarFotos"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(apagado)
                                .truncate()
                                .child("Painel administrativo"),
                        ),
                )
                .into_any_element()
        } else {
            base.size(px(32.))
                .tooltip(|window, cx| Tooltip::new("RecordarFotos — sessões").build(window, cx))
                .into_any_element()
        }
    }

    fn retrato(&self, cx: &Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        div()
            .flex_none()
            .size(px(32.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .justify_center()
            .bg(tema.sidebar_accent)
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .child(
                self.conta
                    .as_ref()
                    .map(Conta::inicial)
                    .unwrap_or_else(|| "·".into()),
            )
    }

    fn nome_e_email(&self, apagado: gpui::Hsla) -> impl IntoElement {
        let (nome, email) = match &self.conta {
            Some(conta) => (conta.exibido().to_string(), conta.email.clone()),
            None => ("Conta RecordarFotos".to_string(), "carregando…".to_string()),
        };
        v_flex()
            .flex_1()
            .min_w(px(0.))
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(nome),
            )
            .child(div().text_xs().text_color(apagado).truncate().child(email))
    }

    fn botao_da_conta(&self, cx: &mut Context<Self>) -> AnyElement {
        let tema = cx.theme();
        let acento = tema.sidebar_accent;
        let apagado = tema.sidebar_foreground.opacity(0.7);
        let aberto = self.menu_da_conta;
        let base = h_flex()
            .id("menu-conta")
            .rounded(px(8.))
            .gap(px(8.))
            .cursor_pointer()
            .hover(|s| s.bg(acento))
            .when(aberto, |d| d.bg(acento))
            .on_click(cx.listener(|raiz, _, _window, cx| raiz.alternar_menu_da_conta(cx)))
            .child(self.retrato(cx));
        if self.menu_aberto {
            base.h(px(48.))
                .p(px(8.))
                .child(self.nome_e_email(apagado))
                .child(Icon::new(Icone::ChevronsUpDown).size(px(16.)))
                .into_any_element()
        } else {
            let nome = self
                .conta
                .as_ref()
                .map(|c| c.exibido().to_string())
                .unwrap_or_else(|| "Conta".into());
            base.size(px(32.))
                .tooltip(move |window, cx| Tooltip::new(nome.clone()).build(window, cx))
                .into_any_element()
        }
    }

    /// A faixa de 56 px: o botão do menu, e o assunto da tela.
    pub(super) fn cabecalho(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tema = cx.theme();
        let (borda, fundo) = (tema.border, tema.background);
        // 🚨 **No Linux esta faixa é a barra de título.** O GNOME não decora
        // janela nenhuma (ver `crate::janela`), e sem isto ela não se move nem
        // maximiza com dois cliques — foi a queixa do dono no Fedora,
        // 17/set/2026. No macOS a chamada não muda nada.
        crate::janela::como_barra_de_titulo(h_flex(), "cabecalho-do-app", window, cx)
            .flex_none()
            .h(px(ALTURA_DO_CABECALHO))
            .px(px(16.))
            .gap(px(8.))
            .border_b_1()
            .border_color(borda)
            .bg(fundo)
            .child(self.botao_do_menu(cx))
            .child(div().w(px(1.)).h(px(16.)).mr(px(4.)).bg(borda))
            .child(
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(8.))
                    .overflow_hidden()
                    .children(self.assunto_do_cabecalho(window, cx)),
            )
    }

    /// O botão que abre e recolhe o menu (`SidebarTrigger`). A galeria usa o
    /// mesmo, na barra dela.
    pub(crate) fn botao_do_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        estilo::botao_do_menu("botao-do-menu", cx).on_click(cx.listener(|raiz, _, _window, cx| {
            raiz.alternar_menu_lateral(cx);
        }))
    }

    fn assunto_do_cabecalho(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let tema = cx.theme();
        let apagado = tema.muted_foreground;
        let titulo = |texto: &'static str| {
            div()
                .flex_none()
                .mr(px(8.))
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child(texto)
        };
        match self.tela {
            Tela::Sessoes => Some(
                h_flex()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(8.))
                    .child(titulo("Sessões fotográficas"))
                    .child(
                        div()
                            .min_w(px(0.))
                            .text_xs()
                            .text_color(apagado)
                            .truncate()
                            .child("O que cada cliente levou no balcão e o que ficou à venda."),
                    )
                    .child(
                        h_flex()
                            .ml_auto()
                            .flex_none()
                            .gap(px(8.))
                            .child(
                                estilo::botao_contorno("cab-espaco", cx)
                                    .child(Icon::new(Icone::HardDrive).size(px(16.)))
                                    .child(div().font_weight(FontWeight::MEDIUM).child("Espaço"))
                                    .child(
                                        div()
                                            .text_color(apagado)
                                            .child(self.espaco_nesta_maquina()),
                                    )
                                    .on_click(cx.listener(|raiz, _, window, cx| {
                                        raiz.abrir_configuracoes(window, cx);
                                    })),
                            )
                            .child(
                                estilo::botao_contorno("cab-retencao", cx)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Retenção")
                                    .on_click(cx.listener(|raiz, _, window, cx| {
                                        raiz.ir_para(Tela::Retencao, window, cx);
                                    })),
                            ),
                    )
                    .into_any_element(),
            ),
            Tela::Impressao => Some(
                h_flex()
                    .flex_1()
                    .gap(px(8.))
                    .child(titulo("Impressão"))
                    .child(
                        div()
                            .text_xs()
                            .text_color(apagado)
                            .child("A folha sai com a foto revelada e enquadrada."),
                    )
                    .child(
                        estilo::botao_contorno("cab-voltar-da-impressao", cx)
                            .ml_auto()
                            .child(Icon::new(Icone::ArrowLeft).size(px(16.)))
                            .child("Voltar à galeria")
                            .on_click(cx.listener(|raiz, _, window, cx| {
                                raiz.voltar_para_biblioteca(window, cx);
                            })),
                    )
                    .into_any_element(),
            ),
            Tela::Biblioteca => Some(
                h_flex()
                    .gap(px(8.))
                    .child(titulo("Catálogo desta máquina"))
                    .into_any_element(),
            ),
            _ => None,
        }
    }

    /// "1,2 GB nesta máquina", do cache de prévias.
    fn espaco_nesta_maquina(&self) -> String {
        let bytes = self.previews.get_stats().total_size_bytes;
        format!(
            "{} nesta máquina",
            crate::configuracoes::formatar_bytes(bytes)
        )
    }

    /// O menu da conta, ao lado do botão dela.
    pub(super) fn menu_da_conta(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let tema = cx.theme();
        let (fundo, borda, apagado, acento, perigo) = (
            tema.popover,
            tema.border,
            tema.muted_foreground,
            tema.accent,
            tema.danger,
        );
        let escolha = self.escolha_de_tema;
        let separador = || div().h(px(1.)).mx(px(-4.)).my(px(4.)).bg(borda);
        let item = |id: &'static str, icone: Icone| {
            h_flex()
                .id(id)
                .h(px(32.))
                .px(px(6.))
                .gap(px(8.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(move |s| s.bg(acento))
                .child(Icon::new(icone).size(px(16.)).text_color(apagado))
        };
        let opcao = |id: &'static str,
                     icone: Icone,
                     rotulo: &'static str,
                     valor: Escolha,
                     cx: &mut Context<Self>| {
            item(id, icone)
                .child(rotulo)
                .when(escolha == valor, |d| {
                    d.child(div().ml_auto().child(Icon::new(Icone::Check).size(px(16.))))
                })
                .on_click(cx.listener(move |raiz, _, window, cx| {
                    raiz.escolher_tema(valor, window, cx);
                }))
        };
        let site = crate::pos_venda::config::ler().site();

        // Uma camada que cobre a janela: clicar fora fecha o menu, como no site.
        div()
            .id("menu-conta-fora")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|raiz, _, _window, cx| {
                    raiz.menu_da_conta = false;
                    cx.notify();
                }),
            )
            .child(
                v_flex()
                    .id("menu-conta-caixa")
                    .absolute()
                    .left(px(self.largura_do_menu() - 4.))
                    .bottom(px(8.))
                    .min_w(px(240.))
                    .p(px(4.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(borda)
                    .bg(fundo)
                    .shadow_md()
                    .text_sm()
                    .text_color(tema.popover_foreground)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .px(px(6.))
                            .py(px(6.))
                            .child(self.retrato(cx))
                            .child(self.nome_e_email(apagado)),
                    )
                    .child(separador())
                    .child(
                        item("conta-loja", Icone::Store)
                            .child("Ver a loja")
                            .on_click(cx.listener(move |raiz, _, _window, cx| {
                                cx.open_url(&format!("{}/loja", site.trim_end_matches('/')));
                                raiz.menu_da_conta = false;
                                cx.notify();
                            })),
                    )
                    .child(separador())
                    .child(
                        div()
                            .px(px(6.))
                            .py(px(4.))
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(apagado)
                            .child("Tema"),
                    )
                    .child(opcao("tema-claro", Icone::Sun, "Claro", Escolha::Claro, cx))
                    .child(opcao(
                        "tema-escuro",
                        Icone::Moon,
                        "Escuro",
                        Escolha::Escuro,
                        cx,
                    ))
                    .child(opcao(
                        "tema-sistema",
                        Icone::Monitor,
                        "Sistema",
                        Escolha::Sistema,
                        cx,
                    ))
                    .child(separador())
                    .child(
                        h_flex()
                            .id("conta-sair")
                            .h(px(32.))
                            .px(px(6.))
                            .gap(px(8.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_color(perigo)
                            .hover(move |s| s.bg(perigo.opacity(0.1)))
                            .child(Icon::new(Icone::LogOut).size(px(16.)))
                            .child("Sair")
                            .on_click(cx.listener(|raiz, _, _window, cx| raiz.sair_da_conta(cx))),
                    ),
            )
    }

    /// O canto de baixo: fotos subindo e o que o site recusou, como a
    /// `FilaDeEnviosDoApp` do Tauri.
    pub(super) fn canto_dos_envios(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let subindo = self.sincronias_pendentes;
        let recusados = self.recusas.len();
        if subindo == 0 && recusados == 0 {
            return None;
        }
        let tema = cx.theme();
        let (fundo, borda, perigo) = (tema.background.opacity(0.95), tema.border, tema.danger);
        let pilula = || {
            h_flex()
                .gap(px(8.))
                .px(px(12.))
                .py(px(8.))
                .rounded(px(10.))
                .border_1()
                .bg(fundo)
                .shadow_lg()
                .text_xs()
        };
        Some(
            v_flex()
                .absolute()
                .left(px(self.largura_do_menu() + 16.))
                .bottom(px(16.))
                .gap(px(8.))
                .when(self.vendo_recusas && recusados > 0, |d| {
                    d.child(
                        v_flex()
                            .id("recusas")
                            .w(px(384.))
                            .max_h(px(320.))
                            .overflow_y_scroll()
                            .p(px(12.))
                            .gap(px(8.))
                            .rounded(px(10.))
                            .border_1()
                            .border_color(borda)
                            .bg(tema.popover)
                            .shadow_md()
                            .text_xs()
                            .children(
                                self.recusas
                                    .iter()
                                    .map(|motivo| div().child(motivo.clone())),
                            ),
                    )
                })
                .child(
                    h_flex()
                        .gap(px(8.))
                        .when(subindo > 0, |d| {
                            d.child(
                                pilula()
                                    .border_color(borda)
                                    .child(Icon::new(Icone::LoaderCircle).size(px(14.)))
                                    .child(if subindo == 1 {
                                        "1 envio na fila".to_string()
                                    } else {
                                        format!("{subindo} envios na fila")
                                    }),
                            )
                        })
                        .when(recusados > 0, |d| {
                            d.child(
                                pilula()
                                    .id("recusas-botao")
                                    .cursor_pointer()
                                    .border_color(perigo.opacity(0.4))
                                    .text_color(perigo)
                                    .child(Icon::new(Icone::TriangleAlert).size(px(14.)))
                                    .child(if recusados == 1 {
                                        "1 envio recusado".to_string()
                                    } else {
                                        format!("{recusados} envios recusados")
                                    })
                                    .on_click(cx.listener(|raiz, _, _window, cx| {
                                        raiz.vendo_recusas = !raiz.vendo_recusas;
                                        cx.notify();
                                    })),
                            )
                        }),
                ),
        )
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn a_conta_sai_do_auth_me_em_qualquer_envelope() {
        let direto = serde_json::json!({"email": "a@b.c", "nome": "Alex S S Fonseca"});
        let conta = Conta::da_resposta(&direto).unwrap();
        assert_eq!(conta.exibido(), "Alex S S Fonseca");
        assert_eq!(conta.inicial(), "A");

        let dentro = serde_json::json!({"data": {"user": {"email": "x@y.z", "nome": "  "}}});
        let conta = Conta::da_resposta(&dentro).unwrap();
        assert_eq!(
            conta.exibido(),
            "x@y.z",
            "sem nome, vale o e-mail, como no site"
        );
        assert_eq!(conta.inicial(), "X");

        assert!(Conta::da_resposta(&serde_json::json!({"ok": true})).is_none());
    }

    #[test]
    fn so_o_caixa_acende_o_caixa() {
        assert_eq!(Tela::Caixa.secao(), Tela::Caixa);
        for tela in [Tela::Sessoes, Tela::Sessao, Tela::Retencao, Tela::Revelacao] {
            assert_eq!(tela.secao(), Tela::Sessoes);
        }
        assert!(!Tela::Revelacao.tem_menu());
        assert!(!Tela::Sessao.tem_cabecalho());
        assert!(Tela::Caixa.tem_cabecalho());
    }
}
