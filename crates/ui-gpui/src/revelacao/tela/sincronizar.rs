//! 🔁 **A caixa do "Sincronizar N"** — as flags do Lightroom, como no site
//! (`sincronizar-dialogo.tsx`, pedido do dono de 2026-09-05: *"coloque flags,
//! escolhe tudo e desmarcar algumas coisas"*).
//!
//! # O desenho (dono, 10/out/2026: *"mais caprichado, usando o GPUI KIT"*)
//!
//! A primeira caixa era uma coluna de texto com "☑" e "☐" escritos à mão, e
//! onze linhas uma embaixo da outra. Agora cada grupo é um **`Checkbox` do
//! kit desenhado como cartão** (o "choice card" do shadcn: a borda e o fundo
//! acendem com a marca quando ligado), em duas colunas, na ordem do site lida
//! em linha. O enquadramento fica à parte, de ponta a ponta, porque não é
//! ajuste — e é o único que costuma estar errado no destino.
//!
//! 📏 **A caixa não muda de tamanho.** A linha de baixo do enquadramento existe
//! sempre: desligado, ela diz por que ele nasce de fora; ligado, vira o alerta.
//! E todo cartão tem a linha de detalhe, para as linhas da grade terem a mesma
//! altura.
//!
//! ⚠️ O diálogo é do kit e mora na `Root` da janela (`open_alert_dialog`) — o
//! mesmo aviso do diálogo de salvar preset.

use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Disableable, Icon, Sizable, WindowExt};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, FontWeight, SharedString, Window};

use super::super::sincronizacao::{self, Escolha, Grupo};
use super::{PedidoDaRevelacao, Revelacao};
use crate::recursos::Icone;

/// A largura da caixa: duas colunas em que o detalhe mais comprido ("as
/// vinhetas pós-corte e do darktable, e o grão") cabe numa linha, e a
/// descrição do alto não deixa uma palavra sozinha na segunda.
const LARGURA: f32 = 720.;

impl Revelacao {
    pub fn abrir_sincronizacao(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let quantas = self.alvos_da_sincronizacao().len();
        if quantas < 2 {
            return;
        }
        // A escolha da última vez, lida do disco uma vez por abertura do app.
        if self.escolha.is_none() {
            self.escolha = Some(sincronizacao::ler());
        }
        let esta = cx.entity();

        // O `AlertDialog` do kit: sem o X e sem fechar pelo véu — um clique
        // perdido fora não descarta o que foi marcado.
        window.open_alert_dialog(cx, move |dialogo, _window, cx| {
            let escolha = esta.read(cx).escolha_da_sincronizacao();
            let para_ok = esta.clone();

            dialogo
                .width(px(LARGURA))
                .icon(selo_do_titulo(cx))
                .title(SharedString::from(format!("Sincronizar {quantas} fotos")))
                .description(
                    "O que estiver marcado vai desta foto para as outras escolhidas na tira. \
                     O resto fica como está em cada uma.",
                )
                .child(
                    v_flex()
                        .gap(px(12.))
                        .pt(px(4.))
                        .child(barra_das_marcas(&esta, &escolha, cx))
                        .child(
                            div().grid().grid_cols(2).gap(px(8.)).children(
                                Grupo::TODOS
                                    .into_iter()
                                    .filter(|grupo| *grupo != Grupo::Enquadramento)
                                    .map(|grupo| cartao(&esta, grupo, &escolha, cx)),
                            ),
                        )
                        .child(cartao(&esta, Grupo::Enquadramento, &escolha, cx).child(
                            aviso_do_enquadramento(escolha.ligado(Grupo::Enquadramento), cx),
                        )),
                )
                .footer(rodape(&esta, quantas, escolha.tem_algo(), cx))
                // 🚨 **Fecha na hora, e devolve `false`.** Deixado à biblioteca,
                // o kit fecha com animação e só devolve o foco depois dela —
                // nesse meio tempo o foco fica num diálogo que já saiu da tela,
                // e a rede da raiz o apanha (o `sincronizar_e_zerar` acusou).
                // O `close_dialog` direto devolve o foco no mesmo quadro.
                .on_ok(move |_ev, window, cx| {
                    confirmar(&para_ok, window, cx);
                    false
                })
                .on_cancel(|_ev, window, cx| {
                    window.close_dialog(cx);
                    false
                })
        });
    }
}

/// O "Sincronizar N" — pelo botão e pelo Enter.
///
/// Nada marcado é nada a fazer: o botão fica desligado, como no site, e o
/// Enter não fecha a caixa.
fn confirmar(esta: &Entity<Revelacao>, window: &mut Window, cx: &mut App) {
    let escolha = esta.read(cx).escolha_da_sincronizacao();
    if !escolha.tem_algo() {
        return;
    }
    sincronizacao::gravar(&escolha);
    esta.update(cx, |_, cx| cx.emit(PedidoDaRevelacao::Sincronizar));
    window.close_dialog(cx);
}

/// O ícone ao lado do título, num quadrado tingido da marca.
fn selo_do_titulo(cx: &App) -> impl IntoElement {
    let marca = cx.theme().primary;
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(36.))
        .rounded(crate::tema::canto(8.))
        .bg(marca.opacity(0.12))
        .text_color(marca)
        .child(Icon::new(Icone::RefreshCw).size(px(18.)))
}

/// Quantos grupos vão, e o atalho de marcar ou desmarcar todos.
fn barra_das_marcas(esta: &Entity<Revelacao>, escolha: &Escolha, cx: &App) -> impl IntoElement {
    let tudo = escolha.tudo();
    let marcados = Grupo::TODOS
        .into_iter()
        .filter(|grupo| escolha.ligado(*grupo))
        .count();
    let para_tudo = esta.clone();
    h_flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .debug_selector(|| "sincronizar-contagem".into())
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(SharedString::from(format!(
                    "{marcados} de {} marcados",
                    Grupo::TODOS.len()
                ))),
        )
        .child(
            crate::estilo::botao_fantasma_pequeno("sincronizar-tudo", cx)
                .icon(Icon::new(if tudo {
                    Icone::Square
                } else {
                    Icone::SquareCheck
                }))
                .label(if tudo {
                    "Desmarcar tudo"
                } else {
                    "Marcar tudo"
                })
                .on_click(move |_ev, _window, cx| {
                    para_tudo.update(cx, |tela, cx| {
                        tela.escolha_mut().marcar_tudo(!tudo);
                        cx.notify();
                    });
                }),
        )
}

/// Um grupo: o `Checkbox` do kit no tamanho de um cartão. O cartão inteiro
/// clica, e a borda e o fundo acendem com a marca quando ele vai.
fn cartao(esta: &Entity<Revelacao>, grupo: Grupo, escolha: &Escolha, cx: &App) -> Checkbox {
    let tema = cx.theme();
    let ligado = escolha.ligado(grupo);
    let marca = tema.primary;
    let id = SharedString::from(format!("sincronizar-{}", grupo.chave()));
    let seletor = id.to_string();
    let para_alternar = esta.clone();
    Checkbox::new(id)
        .debug_selector(move || seletor)
        .small()
        .checked(ligado)
        .label(grupo.rotulo())
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::NORMAL)
                .text_color(tema.muted_foreground)
                .child(grupo.detalhe()),
        )
        .w_full()
        .min_w(px(0.))
        .px(px(12.))
        .py(px(10.))
        .gap(px(10.))
        .rounded(crate::tema::canto(8.))
        .border_1()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .map(|cartao| {
            if ligado {
                cartao
                    .border_color(marca.opacity(0.55))
                    .bg(marca.opacity(0.06))
            } else {
                cartao
                    .border_color(tema.border)
                    .hover(move |s| s.border_color(marca.opacity(0.35)))
            }
        })
        .on_click(move |_marcado, _window, cx| {
            para_alternar.update(cx, |tela, cx| {
                tela.escolha_mut().alternar(grupo);
                cx.notify();
            });
        })
}

/// A linha de baixo do enquadramento, sempre no lugar.
///
/// 🚨 O único grupo que costuma estar errado no destino, e a caixa diz isso
/// quando ele é ligado.
fn aviso_do_enquadramento(ligado: bool, cx: &App) -> impl IntoElement {
    let (cor, texto) = if ligado {
        (
            crate::tema::cores::quente_clara(),
            "O recorte desta foto vale para todas — confira se a composição é a mesma.",
        )
    } else {
        (
            cx.theme().muted_foreground,
            "Fica de fora até você marcar: o recorte de uma foto costuma cortar errado a outra.",
        )
    };
    h_flex()
        .debug_selector(|| "sincronizar-aviso-do-enquadramento".into())
        .gap(px(6.))
        .items_center()
        .text_xs()
        .font_weight(FontWeight::NORMAL)
        .text_color(cor)
        .child(
            Icon::new(if ligado {
                Icone::TriangleAlert
            } else {
                Icone::Info
            })
            .size(px(13.)),
        )
        .child(texto)
}

/// O rodapé: o que acontece depois (o texto do site — sincronizar copia a
/// revelação, e quem leva à galeria é o "Salvar") e os dois botões.
fn rodape(esta: &Entity<Revelacao>, quantas: usize, tem_algo: bool, cx: &App) -> impl IntoElement {
    let para_ok = esta.clone();
    h_flex()
        .w_full()
        .items_center()
        .gap(px(16.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(
                    "A revelação vai para cada foto marcada. Elas sobem para a galeria \
                     quando você salvar.",
                ),
        )
        .child(
            crate::estilo::rodape_do_dialogo()
                .flex_none()
                .child(
                    crate::estilo::botao_contorno("sincronizar-cancelar", cx)
                        .label("Cancelar")
                        .on_click(|_ev, window, cx| window.close_dialog(cx)),
                )
                .child(
                    // O botão diz o que faz, como no site — o kit, sozinho,
                    // escreveria "OK" numa caixa toda em português.
                    crate::estilo::botao_primario("sincronizar-confirmar", cx)
                        .label(SharedString::from(format!("Sincronizar {quantas}")))
                        .disabled(!tem_algo)
                        .on_click(move |_ev, window, cx| confirmar(&para_ok, window, cx)),
                ),
        )
}
