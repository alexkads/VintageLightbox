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
