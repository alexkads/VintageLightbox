//! 🌑 **Os caminhos tenebrosos**: quando a falha cai bem no meio do dinheiro.
//!
//! A regra que manda (dono, 03/out/2026): *"esse sistema não pode haver falhas
//! que interrompam o pagamento do cliente"* — e, do outro lado da mesma regra,
//! nenhuma falha pode cobrar duas vezes nem vender a mesma foto duas vezes.

use super::*;

/// 🌑 **A API grava a venda e a resposta se perde.** O operador vê a falha e
/// tenta de novo: o cliente não pode pagar duas vezes, e o balcão tem de
/// terminar sabendo que a venda existe.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_venda_gravada_com_a_resposta_perdida_nao_cobra_duas_vezes(cx: &mut TestAppContext) {
    let c = preparar(cx, "tenebroso-venda-engolida", 3);
    let (galeria, fotos) = criar_a_sessao(cx, &c, "Venda engolida", "cliente-engolida@e2e.test");
    levar(cx, &c, &galeria, &fotos[..2]);
    abrir_o_caixa(cx, &c, &galeria, "100,00");
    escolher_as_pessoas(cx, &c);
    c.b.ate(cx, "o cupom tem as duas levadas", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.cupom_para_teste().1 == 8000)
    });

    // 🌑 A próxima venda chega à API, é gravada, e a resposta se perde.
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/pos-venda/caixa/vendas", "modo": "engolir", "vezes": 1
    }));
    c.b.teclar(cx, "f4");
    c.b.teclar(cx, "2"); // PIX, já com os 80,00 que faltam
    c.b.teclar(cx, "enter"); // lança
    c.b.teclar(cx, "enter"); // conclui
    c.b.ate_na_api(cx, "a venda foi gravada na API", || {
        let n = vendas_da_galeria(&c, &galeria).len();
        (n == 1).then_some(()).ok_or(format!("{n} vendas"))
    });
    // O balcão diz que não conseguiu — e continua no pagamento, com o PIX
    // lançado: é de lá que o operador tenta de novo.
    c.b.ate(
        cx,
        "o balcão mostra a falha e segura o pagamento",
        |b, cx| {
            b.caixa(cx, |caixa, _w, _cx| {
                caixa.dialogo_do_caixa() == Some("Pagamento")
                    && caixa.pagamentos_lancados() == 1
                    && caixa
                        .avisos_passageiros()
                        .iter()
                        .any(|a| a.contains("Não foi possível registrar"))
            })
        },
    );

    // 🔁 O operador tenta de novo, como faria: Enter.
    c.b.teclar(cx, "enter");
    c.b.ate(
        cx,
        "a segunda tentativa conclui a venda já gravada",
        |b, cx| {
            b.caixa(cx, |caixa, _w, _cx| {
                caixa.ultima_venda_para_teste().is_some() && caixa.dialogo_do_caixa().is_none()
            })
        },
    );

    // 🔑 A garantia: uma venda só, cobrada uma vez.
    let vendas = vendas_da_galeria(&c, &galeria);
    assert_eq!(vendas.len(), 1, "uma venda só: {vendas:?}");
    assert_eq!(vendas[0]["total_centavos"], 8000);
    assert_eq!(
        c.falhas.quantas_vezes("POST", "/pos-venda/caixa/vendas"),
        2,
        "a primeira tentativa (a resposta perdida) e a segunda"
    );
    let numero =
        c.b.caixa(cx, |caixa, _w, _cx| caixa.ultima_venda_para_teste());
    assert_eq!(
        numero.map(serde_json::Value::from),
        Some(vendas[0]["numero"].clone()),
        "o balcão mostra a venda que o servidor gravou"
    );
    let caixa = c
        .site
        .get(&format!("/pos-venda/caixa?estudio_id={}", c.estudio_id));
    let pix: i64 = caixa["vendas"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|v| v["pagamentos"].as_array().cloned().unwrap_or_default())
        .filter(|p| p["forma"] == "pix")
        .map(|p| p["valor_centavos"].as_i64().unwrap_or(0))
        .sum();
    assert_eq!(pix, 8000, "o PIX entrou no caixa uma vez só");
    c.b.ato_limpo(cx, "a venda com a resposta perdida");
}

/// Tenta de novo o diálogo que ficou aberto depois da falha (Enter), ou diz
/// que ele fechou — o que o operador faria.
fn tentar_de_novo(cx: &mut TestAppContext, c: &Cena, dialogo: &str) {
    c.b.ate(
        cx,
        &format!("o balcão segura o {dialogo} depois da falha"),
        |b, cx| {
            b.caixa(cx, |caixa, _w, _cx| {
                caixa.dialogo_do_caixa() == Some(dialogo)
            })
        },
    );
    c.b.teclar(cx, "enter");
}

/// 🌑 **A sangria sai do caixa e a resposta se perde.** Tentar de novo não pode
/// tirar o dinheiro duas vezes — sangria não tem foto que a segure.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_sangria_com_a_resposta_perdida_sai_uma_vez(cx: &mut TestAppContext) {
    let c = preparar(cx, "tenebroso-sangria", 1);
    let (_galeria, _fotos, caixa_id) = ate_o_caixa(cx, &c, "Sangria engolida", 0, "100,00");
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/pos-venda/caixa/movimentos", "modo": "engolir", "vezes": 1
    }));
    pedir_a_sangria(cx, &c, "20,00");
    c.b.ate_na_api(cx, "a sangria foi gravada na API", || {
        let n = caixa_por_id(&c, &caixa_id)["movimentos"]
            .as_array()
            .map_or(0, Vec::len);
        (n == 1).then_some(()).ok_or(format!("{n} movimentos"))
    });
    tentar_de_novo(cx, &c, "Movimento");
    c.b.ate(cx, "a sangria termina no balcão", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.dialogo_do_caixa().is_none())
    });
    let movimentos = caixa_por_id(&c, &caixa_id)["movimentos"].clone();
    assert_eq!(
        movimentos.as_array().map_or(0, Vec::len),
        1,
        "o dinheiro saiu uma vez só: {movimentos}"
    );
    c.b.ato_limpo(cx, "a sangria com a resposta perdida");
}

/// 🌑 **O estorno devolve o dinheiro e a resposta se perde.** Tentar de novo
/// não devolve duas vezes, e o balcão termina mostrando o estorno.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_estorno_com_a_resposta_perdida_devolve_uma_vez(cx: &mut TestAppContext) {
    let c = preparar(cx, "tenebroso-estorno", 2);
    let (galeria, fotos, caixa_id) = ate_o_caixa(cx, &c, "Estorno engolido", 2, "100,00");
    pagar_em_pix(cx, &c);
    let venda = vendas_da_galeria(&c, &galeria).remove(0);
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/estornos", "modo": "engolir", "vezes": 1
    }));
    pedir_o_estorno(cx, &c, &venda, &fotos[0]);
    c.b.ate_na_api(cx, "o estorno foi gravado na API", || {
        let n = caixa_por_id(&c, &caixa_id)["estornos"]
            .as_array()
            .map_or(0, Vec::len);
        (n == 1).then_some(()).ok_or(format!("{n} estornos"))
    });
    tentar_de_novo(cx, &c, "Estorno");
    let numero = venda["numero"].as_i64().unwrap_or_default();
    c.b.ate(cx, "o balcão mostra o estorno", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.dialogo_do_caixa() != Some("Estorno")
                && caixa
                    .vendas_da_sessao_para_teste()
                    .iter()
                    .any(|(n, _, estornado, _)| *n == numero && *estornado == 4000)
        })
    });
    let caixa = caixa_por_id(&c, &caixa_id);
    assert_eq!(
        caixa["estornos"].as_array().map_or(0, Vec::len),
        1,
        "devolveu uma vez só: {caixa}"
    );
    c.b.ato_limpo(cx, "o estorno com a resposta perdida");
}

/// 🌑 **O caixa fecha e a resposta se perde.** Tentar de novo termina com o
/// caixa fechado uma vez, e o balcão sabendo disso.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_fechamento_com_a_resposta_perdida_fecha_uma_vez(cx: &mut TestAppContext) {
    let c = preparar(cx, "tenebroso-fechamento", 1);
    let (_galeria, _fotos, caixa_id) = ate_o_caixa(cx, &c, "Fechamento engolido", 1, "100,00");
    pagar_em_pix(cx, &c);
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/pos-venda/caixa/fechar", "modo": "engolir", "vezes": 1
    }));
    pedir_o_fechamento(cx, &c, "100,00", "40,00");
    c.b.ate_na_api(cx, "o caixa foi fechado na API", || {
        let caixa = caixa_por_id(&c, &caixa_id);
        (!caixa["fechado_em"].is_null())
            .then_some(())
            .ok_or("ainda aberto".to_string())
    });
    tentar_de_novo(cx, &c, "Fechamento");
    c.b.ate(cx, "o balcão termina com o caixa fechado", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.caixa_fechado_no_dialogo() || !caixa.caixa_do_estudio_aberto()
        })
    });
    let caixa = caixa_por_id(&c, &caixa_id);
    for (forma, diferenca) in caixa["diferenca"].as_object().into_iter().flatten() {
        assert_eq!(diferenca, 0, "fechou certo em {forma}: {caixa}");
    }
    c.b.ato_limpo(cx, "o fechamento com a resposta perdida");
}

/// 🌑 **O caixa abre e a resposta se perde.** Tentar de novo termina com o
/// caixa aberto — e um só.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_abertura_com_a_resposta_perdida_abre_uma_vez(cx: &mut TestAppContext) {
    let c = preparar(cx, "tenebroso-abertura", 1);
    let (galeria, _fotos) =
        criar_a_sessao(cx, &c, "Abertura engolida", "cliente-abertura@e2e.test");
    c.b.ate(cx, "o caixa flutuante lê a sessão", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.sessao_escolhida() == Some(galeria.as_str()) && !caixa.carregando()
        })
    });
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/pos-venda/caixa", "modo": "engolir", "vezes": 1
    }));
    c.b.teclar(cx, "f8");
    c.b.digitar(cx, "100,00");
    c.b.teclar(cx, "enter");
    c.b.ate_na_api(cx, "o caixa foi aberto na API", || {
        let aberto = c
            .site
            .get(&format!("/pos-venda/caixa?estudio_id={}", c.estudio_id));
        (!aberto.is_null())
            .then_some(())
            .ok_or("nenhum aberto".to_string())
    });
    tentar_de_novo(cx, &c, "Abrir");
    c.b.ate(cx, "o balcão termina com o caixa aberto", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.caixa_do_estudio_aberto() && caixa.dialogo_do_caixa().is_none()
        })
    });
    let aberto = c
        .site
        .get(&format!("/pos-venda/caixa?estudio_id={}", c.estudio_id));
    assert_eq!(aberto["fundo_de_troco_centavos"], 10000, "{aberto}");
    c.b.ato_limpo(cx, "a abertura com a resposta perdida");
}

/// 🌑 **O token vence no meio do pagamento** (a API responde 401 uma vez). O app
/// renova a sessão e a venda entra — uma vez só.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_token_vencido_no_pagamento_renova_e_vende_uma_vez(cx: &mut TestAppContext) {
    let c = preparar(cx, "tenebroso-token", 1);
    let (galeria, _fotos, _caixa) = ate_o_caixa(cx, &c, "Token vencido", 1, "0,00");
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/pos-venda/caixa/vendas", "modo": "status", "status": 401, "vezes": 1
    }));
    c.b.teclar(cx, "f4");
    c.b.teclar(cx, "2");
    c.b.teclar(cx, "enter");
    c.b.teclar(cx, "enter");
    c.b.ate(cx, "a venda entra depois de renovar a sessão", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.ultima_venda_para_teste().is_some()
        })
    });
    assert_eq!(vendas_da_galeria(&c, &galeria).len(), 1, "uma venda só");
    c.b.ato_limpo(cx, "o token vencido no pagamento");
}
