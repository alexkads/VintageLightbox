//! ⏰ **O portão do caixa do dia** (#52), pela tela e contra a API de verdade
//! (dono, 25/11/2026: *"Vai precisar fazer um teste e2e com teste na interface
//! do caixa"*).
//!
//! O operador do ciclo é `ADMIN` sem perfil, como o balcão de Gramado: passa
//! pelo portão. O cenário percorre os dois estados dele:
//!
//! 1. **Nenhum caixa aberto**: a conta entra e cai no portão; o menu não leva
//!    a outra tela.
//! 2. **Um caixa de anteontem aberto** (aberto pela API com a hora de
//!    anteontem, como um gesto guardado sem rede): o portão pede o
//!    fechamento; sem o motivo, "Contar" recusa; com ele, o fechamento cego
//!    abre com o motivo escrito, confere e fecha, e a API grava o atraso.
//! 3. **O de hoje**: "Abrir o caixa de hoje", o fundo e Enter; o portão solta
//!    e o dia começa na lista de sessões.

use super::*;
use biblioteca_core::caixa::Portao;

/// O seletor do campo do motivo no portão (o `rotulado` do caixa).
const CAMPO_DO_MOTIVO: &str = "caixa-campo-por-que-o-caixa-n-o-foi-fechado-no-hor-rio";

fn portao_agora(c: &Cena, cx: &mut TestAppContext) -> Portao {
    c.b.app(cx, |app, _w, cx| app.caixa.read(cx).portao_agora())
}

#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_portao_fecha_o_caixa_vencido_e_abre_o_de_hoje(cx: &mut TestAppContext) {
    let c = preparar_com_portao(cx, "portao-do-caixa", 0, true);
    let b = &c.b;

    // ── 1. Nenhum caixa aberto: o portão segura ─────────────────────────────
    b.ate(
        cx,
        "a conta cai no portão: abrir o caixa de hoje",
        |b, cx| {
            b.app(cx, |app, _w, cx| {
                app.tela() == Tela::Caixa
                    && app.portao_segura()
                    && app.caixa.read(cx).portao_agora() == Portao::AbrirODeHoje
            })
        },
    );
    b.app(cx, |app, w, cx| app.ir_para(Tela::Sessoes, w, cx));
    b.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Caixa, "o menu não leva a outra tela")
    });

    // ── 2. O caixa de anteontem, aberto e vencido ───────────────────────────
    let (estudio, local) = b.app(cx, |app, _w, cx| {
        let caixa = app.caixa.read(cx);
        (
            caixa
                .estudio_do_caixa()
                .expect("o portão leu o caixa de um estúdio")
                .to_string(),
            biblioteca_core::local::texto_do_caixa(caixa.local_do_caixa()),
        )
    });
    let anteontem = (chrono::Utc::now() - chrono::Duration::days(2)).to_rfc3339();
    let (status, vencido) = c.site.post(
        "/pos-venda/caixa",
        json!({
            "estudio_id": estudio, "fundo_de_troco_centavos": 0,
            "ocorrido_em": anteontem, "local": local,
        }),
    );
    assert!(
        (200..300).contains(&status),
        "abrir o caixa de anteontem: {vencido}"
    );
    assert_eq!(vencido["esquecido"], true, "{vencido}");
    // A tela relê, como quando o operador volta a ela.
    b.app(cx, |app, _w, cx| app.caixa.update(cx, |t, cx| t.abrir(cx)));
    b.ate(
        cx,
        "o portão pede o fechamento do caixa de anteontem",
        |_b, cx| portao_agora(&c, cx) == Portao::FecharOVencido,
    );

    // Sem o motivo, a contagem não começa.
    b.clicar(cx, "caixa-portao-contar");
    b.app(cx, |app, _w, cx| {
        let caixa = app.caixa.read(cx);
        assert_eq!(caixa.dialogo_do_caixa(), None, "sem o motivo, nada abre");
        assert_eq!(
            caixa.erro_do_portao().as_deref(),
            Some(biblioteca_core::caixa::SEM_MOTIVO_DO_ATRASO)
        );
    });

    // Com o motivo, o fechamento cego abre com ele escrito.
    b.clicar(cx, CAMPO_DO_MOTIVO);
    b.digitar(cx, "a última sessão acabou tarde");
    b.clicar(cx, "caixa-portao-contar");
    b.app(cx, |app, _w, cx| {
        assert_eq!(
            app.caixa.read(cx).dialogo_do_caixa(),
            Some("Fechamento"),
            "o motivo leva ao fechamento"
        )
    });
    // A gaveta vazia: o foco nasce no dinheiro; Enter confere, Enter fecha.
    b.digitar(cx, "0,00");
    b.teclar(cx, "enter");
    b.ate(cx, "a conferência mostra o esperado", |b, cx| {
        b.app(cx, |app, _w, cx| app.caixa.read(cx).fechamento_em().0)
    });
    b.teclar(cx, "enter");
    b.ate(cx, "o caixa de anteontem fecha", |b, cx| {
        b.app(cx, |app, _w, cx| app.caixa.read(cx).fechamento_em().1)
    });
    let id = vencido["id"].as_str().expect("o id do caixa vencido");
    let fechado = c.site.get(&format!("/pos-venda/caixa/{id}"));
    assert!(!fechado["fechado_em"].is_null(), "{fechado}");
    assert_eq!(
        fechado["motivo_do_atraso"], "a última sessão acabou tarde",
        "{fechado}"
    );
    b.teclar(cx, "escape");

    // ── 3. O caixa de hoje ──────────────────────────────────────────────────
    b.ate(cx, "o portão passa a pedir o caixa de hoje", |_b, cx| {
        portao_agora(&c, cx) == Portao::AbrirODeHoje
    });
    b.clicar(cx, "caixa-portao-abrir");
    b.app(cx, |app, _w, cx| {
        assert_eq!(app.caixa.read(cx).dialogo_do_caixa(), Some("Abrir"))
    });
    b.digitar(cx, "50,00");
    b.teclar(cx, "enter");
    b.ate(
        cx,
        "o portão solta e o dia começa nas sessões",
        |b, cx| {
            b.app(cx, |app, _w, _cx| {
                !app.portao_segura() && app.tela() == Tela::Sessoes
            })
        },
    );
    let hoje = c.site.get(&format!(
        "/pos-venda/caixa?estudio_id={estudio}&local={local}"
    ));
    assert_eq!(hoje["fundo_de_troco_centavos"], 5000, "{hoje}");
    assert_eq!(hoje["esquecido"], false, "{hoje}");
    b.app(cx, |app, w, cx| app.ir_para(Tela::Caixa, w, cx));
    b.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Caixa));
    b.app(cx, |app, w, cx| app.ir_para(Tela::Sessoes, w, cx));
    b.app(cx, |app, _w, _cx| {
        assert_eq!(
            app.tela(),
            Tela::Sessoes,
            "com o caixa de hoje, o app é livre"
        )
    });

    b.ato_limpo(cx, "o portão do caixa do dia");
}
