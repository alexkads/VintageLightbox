//! A moldura do painel: o menu lateral, o cabeçalho, o menu da conta e o canto
//! dos envios. É o layout do dashboard do site, desenhado aqui com as mesmas
//! medidas.
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
//! | "Backup de arquivos" (no site, em "Conteúdo") | `navegacao.ts` | no menu da conta, e não no lateral |
//! | "N envios na fila" e "N envios recusados", no canto | `FilaDeEnviosDoApp` | [`Aplicativo::canto_dos_envios`] |
//!
//! 🔑 **O que não está pronto no app não aparece nele**: o menu tem só as
//! seções que o app já atende, e nenhuma a mais.

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, Context, FontWeight, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, SharedString, Window,
};

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
    /// O `role` do cadastro (`ADMIN`, `USER`…). Só serve para esconder o que
    /// o backend recusaria — quem autoriza é ele.
    pub papel: Option<String>,
    /// A foto do perfil (a do Google, para quem entra por ele) — a coluna
    /// `avatar` de `users`, a mesma que o menu do site mostra.
    pub avatar: Option<String>,
}

impl Conta {
    /// A conta na resposta de `/auth/me`, esteja ela no topo ou dentro de
    /// `user`/`data` — o servidor já respondeu nos quatro formatos.
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
        let papel = usuario
            .get("role")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let avatar = usuario
            .get("avatar")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|a| a.starts_with("https://") || a.starts_with("http://"))
            .map(str::to_string);
        Some(Conta {
            nome,
            email,
            papel,
            avatar,
        })
    }

    /// 🗑️ É o SuperAdmin — o único que exclui e restaura sessão?
    pub fn e_super_admin(&self) -> bool {
        biblioteca_core::exclusao::e_super_admin(&self.email, self.papel.as_deref())
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

/// As seções que o app tem, em "Operação".
///
/// 📦 O "Backup de arquivos" não está aqui: fica no menu da conta (dono,
/// 2026-09-25: *"não é tão utilizado no dia a dia"*). O menu lateral é o que o
/// balcão usa o dia inteiro.
const MENU: [ItemDoMenu; 4] = [
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
    // 💬 O bot de atendimento — `/dashboard/chatbot`, com o nome e o ícone do
    // menu do site (`navegacao.ts`).
    ItemDoMenu {
        tela: Tela::Chatbot,
        titulo: "Chatbot",
        icone: Icone::Inbox,
    },
    // 📅 A agenda — `/dashboard/agendamentos`, o nome e o ícone do site.
    ItemDoMenu {
        tela: Tela::Agenda,
        titulo: "Agendamentos",
        icone: Icone::CalendarDays,
    },
];

impl Tela {
    /// A seção do menu que fica acesa nesta tela. Tudo o que nasce na lista de
    /// sessões (a galeria, a revelação, a retenção) acende "Sessões".
    fn secao(self) -> Tela {
        match self {
            Tela::Caixa => Tela::Caixa,
            Tela::Backup => Tela::Backup,
            Tela::Chatbot => Tela::Chatbot,
            Tela::Agenda => Tela::Agenda,
            _ => Tela::Sessoes,
        }
    }

    /// Se a tela tem a faixa do cabeçalho. A galeria tem a barra dela.
    pub(super) fn tem_cabecalho(self) -> bool {
        !matches!(self, Tela::Revelacao | Tela::Sessao | Tela::NovaSessao)
    }
}

impl Aplicativo {
    /// O número de conversas com novidade, no item "Chatbot" do menu — e o
    /// vermelho quando há quem pediu atendente. `None` quando não há nada.
    fn selo_do_chatbot(&self, aberto: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        let chatbot = self.chatbot.read(cx);
        let (novidades, esperando) = (chatbot.quantas_novidades(), chatbot.alguem_esperando());
        if novidades == 0 && !esperando {
            return None;
        }
        let cor: gpui_kit::Hsla = if esperando {
            gpui_kit::rgb(0xef4444).into()
        } else {
            gpui_kit::rgb(0x16a34a).into()
        };
        let selo = div()
            .id("menu-selo-do-chatbot")
            .debug_selector(|| "menu-selo-do-chatbot".into())
            .flex()
            .items_center()
            .justify_center()
            .min_w(px(16.))
            .h(px(16.))
            .px(px(4.))
            .rounded_full()
            .bg(cor)
            .text_color(gpui_kit::white())
            .text_xs()
            .child(if novidades > 0 {
                novidades.to_string()
            } else {
                "!".into()
            });
        Some(if aberto {
            div()
                .absolute()
                .right(px(8.))
                .child(selo)
                .into_any_element()
        } else {
            div()
                .absolute()
                .top(px(-2.))
                .right(px(-2.))
                .child(selo)
                .into_any_element()
        })
    }

    /// Troca de tela pelo menu, pelo cabeçalho ou pela barra de uma tela.
    ///
    /// 🔑 **Sair da galeria é sair da sessão**, como mudar de rota no site: a
    /// grade, a fila e as teclas de triagem deixam de valer para o ensaio que
    /// estava aberto. A revelação e a impressão são de dentro da galeria, e
    /// não passam por aqui.
    pub fn ir_para(&mut self, tela: Tela, window: &mut Window, cx: &mut Context<Self>) {
        self.menu_da_conta = false;
        // 💬 O chatbot só relê com a tela na frente; escondido, ele só acende
        // as novidades e avisa.
        if (self.tela == Tela::Chatbot) != (tela == Tela::Chatbot) {
            let visivel = tela == Tela::Chatbot;
            self.chatbot.update(cx, |t, cx| t.mostrar(visivel, cx));
        }
        if (self.tela == Tela::Agenda) != (tela == Tela::Agenda) {
            let visivel = tela == Tela::Agenda;
            self.agenda.update(cx, |t, cx| t.mostrar(visivel, cx));
        }
        // 🎞️ O menu lateral também está na Revelação: sair dela por ele grava
        // o pendente, e a guia da sessão guarda a Revelação para voltar nela.
        // Para a galeria da mesma sessão é só sair: a guia fica na frente.
        if tela == Tela::Sessao {
            self.largar_a_revelacao(cx);
        } else {
            self.estacionar_a_revelacao(cx);
        }
        if tela != Tela::Sessao && self.sessao_aberta.is_some() {
            self.sair_da_sessao(cx);
        }
        match tela {
            Tela::Sessoes => self.sessoes.update(cx, |t, cx| t.recarregar(cx)),
            Tela::Caixa => self.caixa.update(cx, |t, cx| t.abrir(cx)),
            Tela::Retencao => self.retencao.update(cx, |t, cx| t.abrir(window, cx)),
            Tela::NovaSessao => self.nova_sessao.update(cx, |t, cx| t.abrir(window, cx)),
            // 🔑 **A sessão chega aqui, e não na entrada.** Listar o acervo na
            // hora do login gastaria um pedido por abertura do app para uma
            // tela que quase nunca se abre; e a conta pode trocar sem o app
            // fechar. Aqui a tela recebe a conta de agora e lê a raiz.
            Tela::Backup => {
                if let Some(sessao) = self.sessao.clone() {
                    self.backup
                        .update(cx, |t, cx| t.com_sessao(sessao, window, cx));
                }
            }
            _ => {}
        }
        self.tela = tela;
        window.focus(&self.foco, cx);
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

    /// Claro, Escuro, Sistema, Matrix ou Cyberpunk — e a escolha fica para a
    /// próxima abertura.
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
        self.largar_a_revelacao(cx);
        if self.sessao_aberta.is_some() {
            self.sair_da_sessao(cx);
        }
        // As guias são da conta que sai: a próxima não herda os clientes dela.
        self.guias.esvaziar();
        self.guardar_guias();
        let (canal, _) = std::sync::mpsc::channel();
        self.publicador.sair(canal);
        // Os fluxos do chatbot eram da conta que saiu.
        self.chatbot.update(cx, |t, cx| {
            t.mostrar(false, cx);
            t.sair(cx);
        });
        self.agenda.update(cx, |t, cx| {
            t.mostrar(false, cx);
            t.sair(cx);
        });
        self.sessao = None;
        self.conta = None;
        self.retrato = None;
        self._retrato = None;
        self.sessoes
            .update(cx, |t, cx| t.definir_super_admin(false, cx));
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

    /// 🖼️ **A foto do perfil**, como no menu do site. Baixada uma vez por
    /// entrada, numa thread: o cliente é bloqueante e a interface não espera.
    /// Falhou (a URL do Google expira quando a pessoa troca a foto, ou a rede
    /// caiu), fica a inicial — o `AvatarFallback` do site.
    fn baixar_o_retrato(&mut self, cx: &mut Context<Self>) {
        self.retrato = None;
        let Some(endereco) = self.conta.as_ref().and_then(|c| c.avatar.clone()) else {
            self._retrato = None;
            return;
        };
        let baixando = cx
            .background_executor()
            .spawn(async move { retrato_de(&endereco) });
        self._retrato = Some(cx.spawn(async move |raiz, cx| {
            let Some(retrato) = baixando.await else {
                return;
            };
            let _ = raiz.update(cx, |raiz, cx| {
                raiz.retrato = Some(retrato);
                cx.notify();
            });
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
                    Ok(valor) => {
                        self.conta = Conta::da_resposta(&valor);
                        self.baixar_o_retrato(cx);
                        // 🗑️ A lixeira da lista só aparece para o SuperAdmin,
                        // e só agora se sabe quem entrou.
                        let pode = self.conta.as_ref().is_some_and(Conta::e_super_admin);
                        self.sessoes
                            .update(cx, |t, cx| t.definir_super_admin(pode, cx));
                        // 🗂️ As guias da última abertura voltam, se forem
                        // desta conta — só agora se sabe quem entrou.
                        self.repor_guias();
                    }
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
                    // Para o teste clicar onde o dedo clica.
                    .debug_selector(move || format!("menu-{titulo}"))
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
                // 💬 O chatbot mostra quantas conversas têm mensagem nova, e
                // pinta de vermelho quando alguém pediu atendente.
                let selo = (item.tela == Tela::Chatbot)
                    .then(|| self.selo_do_chatbot(aberto, cx))
                    .flatten();
                let base = base.relative().children(selo);
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

    fn retrato(&self, cx: &Context<Self>) -> AnyElement {
        if let Some(retrato) = self.retrato.clone() {
            return gpui_kit::img(retrato)
                .flex_none()
                .size(px(32.))
                .rounded(px(8.))
                .object_fit(gpui_kit::ObjectFit::Cover)
                .into_any_element();
        }
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
            .into_any_element()
    }

    fn nome_e_email(&self, apagado: gpui_kit::Hsla) -> impl IntoElement {
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
        let (borda, fundo, frente) = (tema.border, tema.background, tema.foreground);
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
            .bg(crate::janela::fundo_da_barra(fundo, window, cx))
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
            // 🪟 **No Linux os botões de janela são estes.** Sem barra do
            // sistema, fechar e minimizar só existem se o app os desenhar; fora
            // do Linux a chamada não devolve nada.
            .child(crate::janela::controles_da_tela(
                "janela-app",
                frente,
                window,
                cx,
            ))
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
                // 🔑 `flex_none`: com algo à direita (a versão, o ✓) o
                // flex encolhia a caixa do ícone, o desenho de 16 px invadia
                // o espaço e o texto encostava nele (dono, 2026-09-26).
                .child(
                    Icon::new(icone)
                        .size(px(16.))
                        .flex_none()
                        .text_color(apagado),
                )
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
                    // 📦 O acervo de arquivos no R2 — `/dashboard/backup` no
                    // site, portado do `file-manager` do legado em 2026-09-18.
                    // Aqui, e não no menu lateral: é de uso de vez em quando.
                    .child(
                        item("conta-backup", Icone::FolderOpen)
                            .debug_selector(|| "conta-backup".into())
                            .child("Backup de arquivos")
                            .on_click(cx.listener(|raiz, _, window, cx| {
                                raiz.ir_para(Tela::Backup, window, cx);
                            })),
                    )
                    // 🔄 A procura da abertura é silenciosa; esta responde
                    // sempre, na faixa do rodapé — inclusive "está em dia".
                    .child(
                        item("conta-atualizacoes", Icone::RefreshCw)
                            .debug_selector(|| "conta-atualizacoes".into())
                            .child("Verificar atualizações")
                            .child(
                                div()
                                    .ml_auto()
                                    .text_xs()
                                    .text_color(apagado)
                                    .child(concat!("v", env!("CARGO_PKG_VERSION"))),
                            )
                            .on_click(cx.listener(|raiz, _, _window, cx| {
                                raiz.menu_da_conta = false;
                                raiz.verificar_atualizacoes(cx);
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
                    .child(opcao(
                        "tema-matrix",
                        Icone::SquareTerminal,
                        "Matrix",
                        Escolha::Matrix,
                        cx,
                    ))
                    .child(opcao(
                        "tema-cyberpunk",
                        Icone::Zap,
                        "Cyberpunk",
                        Escolha::Cyberpunk,
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
                            .child(Icon::new(Icone::LogOut).size(px(16.)).flex_none())
                            .child("Sair")
                            .on_click(cx.listener(|raiz, _, _window, cx| raiz.sair_da_conta(cx))),
                    ),
            )
    }

    /// O canto de baixo: fotos subindo e o que o site recusou.
    ///
    /// 🔑 **Arrasta-se pela alça**, como o caixa — ver
    /// `app/canto_dos_envios.rs`. Nasce no pé, ao lado do menu, que é onde a
    /// tira desenha as primeiras fotos.
    pub(super) fn canto_dos_envios(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        use super::canto_dos_envios::{dentro_dos_limites, MARGEM};

        let subindo = self.sincronias_pendentes;
        let recusados = self.recusas.len();
        if subindo == 0 && recusados == 0 {
            return None;
        }
        let menu = self.largura_do_menu();
        let janela = window.viewport_size();
        let area = (f32::from(janela.width) - menu, f32::from(janela.height));
        // Com a janela menor, o canto volta para dentro.
        let (x, y) = dentro_dos_limites(self.canto.posicao, self.canto.tamanho.get(), area);
        let tamanho = self.canto.tamanho.clone();
        let tema = cx.theme();
        let (fundo, borda, perigo, apagado) = (
            tema.background.opacity(0.95),
            tema.border,
            tema.danger,
            tema.muted_foreground,
        );
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
        let alca = h_flex()
            .id("canto-dos-envios-alca")
            .debug_selector(|| "canto-dos-envios-alca".into())
            .cursor_move()
            .px(px(4.))
            .py(px(8.))
            .rounded(px(10.))
            .border_1()
            .border_color(borda)
            .bg(fundo)
            .shadow_lg()
            .child(
                Icon::new(Icone::GripVertical)
                    .size(px(14.))
                    .text_color(apagado),
            )
            .tooltip(|window, cx| Tooltip::new("Arraste para mudar de lugar").build(window, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|raiz, e: &MouseDownEvent, _window, cx| {
                    raiz.canto
                        .comecar((f32::from(e.position.x), f32::from(e.position.y)));
                    cx.notify();
                }),
            );
        // 🔑 **O arrasto escuta a janela inteira**, e não a alça: o ponteiro
        // sai de cima dela no primeiro pixel. É o mesmo desenho do caixa.
        let arrasto = self.canto.arrastando().then(|| {
            let fraca = cx.entity().downgrade();
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let para_mover = fraca.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, _, _, cx| {
                        if let Some(raiz) = para_mover.upgrade() {
                            raiz.update(cx, |raiz, cx| {
                                let ponto = (f32::from(e.position.x), f32::from(e.position.y));
                                if raiz.canto.arrastar(ponto, area) {
                                    cx.notify();
                                }
                            });
                        }
                    });
                    let para_soltar = fraca.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, _, _, cx| {
                        if let Some(raiz) = para_soltar.upgrade() {
                            raiz.update(cx, |raiz, cx| {
                                if raiz.canto.soltar() {
                                    cx.notify();
                                }
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_full()
        });
        let canto = v_flex()
            .id("canto-dos-envios")
            .debug_selector(|| "canto-dos-envios".into())
            .absolute()
            .left(px(menu + MARGEM + x))
            .bottom(px(MARGEM - y))
            .gap(px(8.))
            .child(
                h_flex()
                    .gap(px(8.))
                    .child(alca)
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
            )
            .child(
                canvas(
                    move |limites, _, _| {
                        tamanho.set((
                            f32::from(limites.size.width),
                            f32::from(limites.size.height),
                        ))
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        Some(div().absolute().inset_0().children(arrasto).child(canto))
    }
}

/// Baixa e decodifica a foto do perfil. **Bloqueia**: chamar fora da thread da
/// interface.
fn retrato_de(endereco: &str) -> Option<std::sync::Arc<gpui_kit::RenderImage>> {
    let cliente = cargo_packager_updater::reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .ok()?;
    let resposta = cliente.get(endereco).send().ok()?;
    if !resposta.status().is_success() {
        eprintln!(
            "⚠️ [Conta] a foto do perfil respondeu {}",
            resposta.status()
        );
        return None;
    }
    let bytes = resposta.bytes().ok()?;
    let imagem = image::load_from_memory(&bytes).ok()?;
    // 64 px bastam para um retrato de 32 em tela Retina.
    let imagem = imagem.thumbnail(64, 64);
    Some(crate::imagem::para_gpui(imagem))
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
    fn a_foto_do_perfil_vem_do_avatar_e_so_se_for_endereco() {
        let com_foto = serde_json::json!({"user": {
            "email": "alexkads@gmail.com",
            "avatar": "https://lh3.googleusercontent.com/a/abc=s96-c"
        }});
        assert_eq!(
            Conta::da_resposta(&com_foto).unwrap().avatar.as_deref(),
            Some("https://lh3.googleusercontent.com/a/abc=s96-c")
        );
        for sem in [
            serde_json::json!({"email": "a@b.c"}),
            serde_json::json!({"email": "a@b.c", "avatar": null}),
            serde_json::json!({"email": "a@b.c", "avatar": "  "}),
            serde_json::json!({"email": "a@b.c", "avatar": "data:image/png;base64,AAAA"}),
        ] {
            assert_eq!(Conta::da_resposta(&sem).unwrap().avatar, None, "{sem}");
        }
    }

    #[test]
    fn o_papel_do_auth_me_decide_quem_e_super_admin() {
        let dono = serde_json::json!({"email": "alexkads@gmail.com", "role": "ADMIN"});
        assert!(Conta::da_resposta(&dono).unwrap().e_super_admin());
        let funcionario = serde_json::json!({"email": "caixa@loja.com", "role": "ADMIN"});
        assert!(!Conta::da_resposta(&funcionario).unwrap().e_super_admin());
        let sem_papel = serde_json::json!({"email": "alexkads@gmail.com"});
        assert!(!Conta::da_resposta(&sem_papel).unwrap().e_super_admin());
    }

    #[test]
    fn so_o_caixa_acende_o_caixa() {
        assert_eq!(Tela::Caixa.secao(), Tela::Caixa);
        for tela in [Tela::Sessoes, Tela::Sessao, Tela::Retencao, Tela::Revelacao] {
            assert_eq!(tela.secao(), Tela::Sessoes);
        }
        assert!(!Tela::Sessao.tem_cabecalho());
        assert!(Tela::Caixa.tem_cabecalho());
    }
}
