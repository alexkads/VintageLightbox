//! 🔊 Os avisos sonoros pelo caminho de verdade: o evento chega pelo tempo
//! real, a raiz avisa, e o alto-falante recebe o pedido certo — e as
//! Configurações mudam isso pelo clique.

use gpui_kit::TestAppContext;
use serde_json::json;

use super::agenda;
use super::chatbot::{self, clicar, desenhado};
use super::Estudio;
use crate::chatbot::modelo::fuso_do_estudio;
use crate::sons::{Embutido, Evento, Modo, Som, Sons};
use crate::tempo_real::porta::Sinal;

fn tocados(e: &Estudio) -> Vec<Option<Som>> {
    e.sons.pedidos().into_iter().map(|p| p.som).collect()
}

fn mudar(
    e: &Estudio,
    cx: &mut TestAppContext,
    mudanca: impl FnOnce(&mut crate::sons::Preferencias),
) {
    e.app(cx, |_, _, cx| cx.global_mut::<Sons>().mudar(mudanca));
}

/// Mensagem de cliente com o operador em outra tela: o sino.
#[gpui_kit::test]
fn mensagem_de_cliente_toca_o_sino(cx: &mut TestAppContext) {
    let e = chatbot::entrar(cx);
    e.escuta.mandar(chatbot::mensagem_da_ana("oi, tudo bem?"));
    e.esperar(cx);
    assert_eq!(tocados(&e), vec![Some(Som::Embutido(Embutido::Sino))]);
}

/// Com a conversa aberta na frente, o operador já está lendo: nem toast,
/// nem som — a mesma regra.
#[gpui_kit::test]
fn a_conversa_aberta_nao_toca(cx: &mut TestAppContext) {
    let e = chatbot::abrir_o_chatbot(cx);
    clicar(&e, cx, "chatbot-conversa-0");
    e.escuta.mandar(chatbot::mensagem_da_ana("parte 1"));
    e.esperar(cx);
    assert!(e.sons.pedidos().is_empty());
}

/// Novo e cancelado têm sons próprios, e a voz lê o cliente e o horário.
#[gpui_kit::test]
fn agendamento_novo_e_cancelado_tem_sons_proprios_e_a_voz_le_o_horario(cx: &mut TestAppContext) {
    let e = agenda::entrar(cx);
    mudar(&e, cx, |p| {
        p.escolha_mut(Evento::NovoAgendamento).modo = Modo::SomEVoz;
    });
    let quando = agenda::as_(1, 9, 0);
    let horario = quando
        .with_timezone(&fuso_do_estudio())
        .format("%d/%m, %H:%M")
        .to_string();

    e.escuta.mandar(agenda::criado("e9", quando, Some("s1")));
    e.esperar(cx);
    e.escuta.mandar(Sinal::Evento {
        fonte: "agenda",
        dados: json!({"tipo": "ensaio_cancelado", "ensaio_id": "e2", "quando": null,
                      "estudio_id": null, "origem": null, "cliente": "Ana e Bia"}),
    });
    e.esperar(cx);

    let pedidos = e.sons.pedidos();
    assert_eq!(pedidos.len(), 2);
    assert_eq!(pedidos[0].som, Some(Som::Embutido(Embutido::DoisToques)));
    assert_eq!(
        pedidos[0].fala.as_ref().map(|f| f.texto.clone()),
        Some(format!("Novo agendamento: Família Nova · {horario}"))
    );
    assert_eq!(pedidos[1].som, Some(Som::Embutido(Embutido::Descida)));
    assert_eq!(pedidos[1].fala, None, "o cancelado ficou só no som");
}

/// 🚨 As Configurações pelo clique: a aba dos sons abre primeiro, o
/// interruptor do tipo cala aquele aviso, e a aba do cache continua lá.
#[gpui_kit::test]
fn as_configuracoes_desligam_um_aviso_pelo_clique(cx: &mut TestAppContext) {
    let e = agenda::entrar(cx);
    e.app(cx, |app, window, cx| app.abrir_configuracoes(window, cx));
    assert!(desenhado(&e, cx, "aba-avisos-sonoros"));
    assert!(desenhado(&e, cx, "som-ligado-novo_agendamento"));

    clicar(&e, cx, "som-ligado-novo_agendamento");
    e.app(cx, |_, _, cx| {
        assert!(
            !cx.global::<Sons>()
                .preferencias()
                .escolha(Evento::NovoAgendamento)
                .ligado
        );
    });
    e.escuta
        .mandar(agenda::criado("e9", agenda::as_(1, 9, 0), Some("s1")));
    e.esperar(cx);
    assert!(e.sons.pedidos().is_empty(), "desligado não toca");

    clicar(&e, cx, "ouvir-novo_agendamento");
    assert_eq!(
        tocados(&e),
        vec![Some(Som::Embutido(Embutido::DoisToques))],
        "o Ouvir toca mesmo desligado"
    );

    clicar(&e, cx, "aba-cache");
    e.app(cx, |app, _, cx| {
        assert_eq!(app.configuracoes_para_teste().read(cx).aba(), 1)
    });
}

/// ⚙️ As Configurações se abrem pelo menu da conta, já nos avisos sonoros
/// (dono, 03/out/2026: *"Deveria ficar aqui!"*); o "Espaço" do cabeçalho
/// das sessões abre a mesma janela no cache.
#[gpui_kit::test]
fn as_configuracoes_se_abrem_pelo_menu_da_conta(cx: &mut TestAppContext) {
    let e = super::abrir_o_app(cx, super::Cenario::default());
    e.entrar_na_conta(cx);

    e.app(cx, |app, window, cx| app.alternar_menu_da_conta(window, cx));
    clicar(&e, cx, "conta-configuracoes");
    e.app(cx, |app, _, cx| {
        assert!(app.configurando());
        assert!(!app.menu_da_conta_aberto(), "abrir fecha o menu");
        assert_eq!(app.configuracoes_para_teste().read(cx).aba(), 0);
    });
    assert!(desenhado(&e, cx, "som-ligado-falha"));

    e.app(cx, |app, window, cx| app.fechar_configuracoes(window, cx));
    clicar(&e, cx, "cab-espaco");
    e.app(cx, |app, _, cx| {
        assert!(app.configurando());
        assert_eq!(
            app.configuracoes_para_teste().read(cx).aba(),
            1,
            "o Espaço abre no cache"
        );
    });
}
