//! O que a raiz conta à bandeja (`crate::segundo_plano`), e o que a bandeja
//! pede a ela.

use gpui::{div, prelude::*, px, App, Context};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable};

use super::Aplicativo;
use crate::segundo_plano::frases::Retrato;
use crate::tema;

/// O aviso de sair com envio no ar, onde o sistema não tem bandeja.
///
/// # 🚨 Por que existe
///
/// Fechar leva à bandeja (dono, 21/set/2026), para os envios continuarem. Mas
/// o GNOME sem a extensão AppIndicator **não mostra bandeja**: fechar
/// minimizava para sempre, o "Sair" não aparecia, e o app não tinha como ser
/// encerrado (dono, 25/set/2026). Fechar calado, com foto subindo, também não
/// serve — daí a pergunta, e só quando há o que perder de vista (dono, no
/// mesmo dia: *"perguntar só se houver envio"*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saida {
    /// As três escolhas na tela.
    Perguntando,
    /// "Esperar terminar e sair": o app sai sozinho quando a fila esvaziar.
    Esperando,
}

impl Aplicativo {
    /// O retrato do trabalho em segundo plano, lido a cada volta da bandeja.
    ///
    /// ⚠️ Sem o que mora fora da raiz (tamanho do catálogo, pilha local, a
    /// hora): isso é o laço quem sabe.
    pub(crate) fn retrato_do_segundo_plano(&self, cx: &App) -> Retrato {
        // ❌ **As rejeitadas são as que não vão subir** (contrato C21): foto de
        // um ensaio, ainda fora do site, com a bandeira do `X`.
        //
        // 🔄 **Era "esperando nota"**, e contava a foto sem nota da sessão —
        // certo até 2026-09-20, quando a nota era a porta da nuvem. Hoje a sem
        // nota sobe como qualquer outra, e contá-la aqui diria ao operador que
        // há trabalho parado quando não há.
        let rejeitadas = if self.conta.is_some() {
            self.biblioteca.read(cx).contar_fotos(|f| {
                f.sessao_id.is_some()
                    && f.pos_venda_foto_id.is_none()
                    && f.flag == Some(crate::biblioteca::marcacao::REJEITADA_NO_CATALOGO)
            })
        } else {
            0
        };
        Retrato {
            subindo: self.sincronias_pendentes,
            rejeitadas,
            recusadas: self.recusas.len(),
            refazendo: self.reposicoes_pendentes,
            conta: self.conta.as_ref().map(|c| c.email.clone()),
            ultimo_envio: self.ultimo_envio,
            ..Default::default()
        }
    }

    /// O pedido de fechar sem bandeja no sistema, com envio no ar.
    pub(crate) fn perguntar_antes_de_sair(&mut self, cx: &mut Context<Self>) {
        self.saida = Some(Saida::Perguntando);
        cx.notify();
    }

    #[cfg(test)]
    pub(crate) fn saida_para_teste(&self) -> Option<Saida> {
        self.saida
    }

    /// "Esperar terminar e sair".
    pub(crate) fn esperar_a_fila_e_sair(&mut self, cx: &mut Context<Self>) {
        self.saida = Some(Saida::Esperando);
        crate::segundo_plano::sair_quando_a_fila_esvaziar(true, cx);
        cx.notify();
    }

    /// Desiste de sair — "Cancelar" nas duas fases do aviso.
    pub(crate) fn desistir_de_sair(&mut self, cx: &mut Context<Self>) {
        self.saida = None;
        crate::segundo_plano::sair_quando_a_fila_esvaziar(false, cx);
        cx.notify();
    }

    /// O cartão por cima de tudo, no formato do "Tirar fotos do catálogo?".
    pub(super) fn aviso_de_saida(&self, saida: Saida, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.sincronias_pendentes;
        let fotos = if n == 1 {
            "1 envio".to_string()
        } else {
            format!("{n} envios")
        };
        let (titulo, texto) = match saida {
            Saida::Perguntando => (
                format!("{fotos} ainda subindo para o site"),
                "Este computador não tem bandeja do sistema, onde o app ficaria \
                 terminando os envios. Se sair agora, o que não subiu fica \
                 guardado e sobe na próxima vez que o VintageLightbox abrir."
                    .to_string(),
            ),
            Saida::Esperando => (
                format!("Saindo quando terminar — faltam {fotos}"),
                "O app fecha sozinho assim que o último envio chegar ao site.".to_string(),
            ),
        };
        let botoes = match saida {
            Saida::Perguntando => div()
                .flex()
                .flex_wrap()
                .justify_end()
                .gap(px(8.))
                .child(
                    Button::new("saida-cancelar")
                        .label("Cancelar")
                        .xsmall()
                        .on_click(cx.listener(|app, _ev, _w, cx| app.desistir_de_sair(cx))),
                )
                .child(
                    Button::new("saida-sair-agora")
                        .label("Sair mesmo assim")
                        .xsmall()
                        .on_click(|_ev, _w, cx| crate::segundo_plano::sair_agora(cx)),
                )
                .child(
                    Button::new("saida-minimizar")
                        .label("Minimizar e continuar")
                        .xsmall()
                        .on_click(cx.listener(|app, _ev, window, cx| {
                            app.desistir_de_sair(cx);
                            window.minimize_window();
                        })),
                )
                .child(
                    Button::new("saida-esperar")
                        .label("Esperar terminar e sair")
                        .xsmall()
                        .primary()
                        .on_click(cx.listener(|app, _ev, _w, cx| app.esperar_a_fila_e_sair(cx))),
                ),
            Saida::Esperando => div()
                .flex()
                .justify_end()
                .gap(px(8.))
                .child(
                    Button::new("saida-cancelar")
                        .label("Não sair")
                        .xsmall()
                        .on_click(cx.listener(|app, _ev, _w, cx| app.desistir_de_sair(cx))),
                )
                .child(
                    Button::new("saida-sair-agora")
                        .label("Sair agora")
                        .xsmall()
                        .on_click(|_ev, _w, cx| crate::segundo_plano::sair_agora(cx)),
                ),
        };

        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(tema::cores::veu())
            .child(
                div()
                    .id("aviso-de-saida")
                    .debug_selector(|| "aviso-de-saida".to_string())
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .p(px(16.))
                    .max_w(px(520.))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(px(6.))
                    .child(div().text_sm().child(titulo))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(texto),
                    )
                    .child(botoes),
            )
    }

    /// Fecha a segunda tela, se aberta. A janela principal foi para a bandeja,
    /// e o monitor do cliente não pode ficar com a tela sozinha.
    pub(crate) fn fechar_tela_do_cliente(&mut self, cx: &mut Context<Self>) {
        if let Some(janela) = self.cliente.take() {
            let _ = janela.update(cx, |_cliente, window, _cx| window.remove_window());
            self.detalhe
                .update(cx, |tela, cx| tela.definir_cliente_aberta(false, cx));
        }
    }
}

#[cfg(test)]
mod testes {
    use gpui::TestAppContext;

    use crate::app::testes::{acervo, portas, previews_descartaveis};
    use crate::app::Aplicativo;

    #[gpui::test]
    fn o_retrato_conta_o_que_a_raiz_espera_do_site(cx: &mut TestAppContext) {
        let (previews, _dir) = previews_descartaveis();
        cx.update(gpui_component::init);
        let janela = cx.add_window(|window, cx| {
            Aplicativo::ja_dentro(acervo(), previews, Vec::new(), portas(), window, cx)
        });
        janela
            .update(cx, |app, _window, cx| {
                let r = app.retrato_do_segundo_plano(cx);
                assert_eq!(r.subindo, 0);
                assert_eq!(r.recusadas, 0);
                assert!(!r.ha_envio_pendente());

                app.sincronias_pendentes = 2;
                app.recusas.push("410: foto apagada".into());
                app.ultimo_envio = Some(123);
                let r = app.retrato_do_segundo_plano(cx);
                assert_eq!(r.subindo, 2);
                assert_eq!(r.recusadas, 1);
                assert_eq!(r.ultimo_envio, Some(123));
                assert!(r.ha_envio_pendente(), "fechar agora esconderia a janela");
            })
            .expect("a janela deve estar aberta");
    }
}
