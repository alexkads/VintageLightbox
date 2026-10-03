//! 😕 **Os caminhos tristes**: o operador ou o cliente erram, e o sistema
//! recusa com clareza e deixa terminar do jeito certo — sem gravar o errado,
//! sem travar e sem sumir com o que já estava feito.

use super::*;

/// O seletor que o `rotulado` do caixa dá ao campo: o rótulo em minúsculas
/// ASCII, com `-` no lugar do resto (`caixa::dialogos::seletor_do_rotulo`).
fn campo_do_caixa(rotulo: &str) -> String {
    let mut saida = String::new();
    for c in rotulo.chars() {
        if c.is_ascii_alphanumeric() {
            saida.push(c.to_ascii_lowercase());
        } else if !saida.ends_with('-') {
            saida.push('-');
        }
    }
    format!("caixa-campo-{}", saida.trim_matches('-'))
}

fn erro_no_dialogo(c: &Cena, cx: &mut TestAppContext) -> Option<String> {
    c.b.caixa(cx, |caixa, _w, _cx| caixa.erro_no_dialogo_para_teste())
}

fn avisos(c: &Cena, cx: &mut TestAppContext) -> Vec<String> {
    let mut todos: Vec<String> =
        c.b.app(cx, |app, _w, _cx| app.avisos_dados_para_teste())
            .into_iter()
            .map(|(t, _)| t)
            .collect();
    todos.extend(c.b.caixa(cx, |caixa, _w, _cx| caixa.avisos_passageiros()));
    todos
}

/// 😕 **Pagamento menor que o total**: o PIX cobre só parte, e o Enter de
/// concluir não fecha a venda — o balcão diz quanto falta. O dinheiro completa,
/// e a venda entra com as duas formas.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_pagamento_menor_que_o_total_nao_conclui_e_diz_quanto_falta(cx: &mut TestAppContext) {
    let c = preparar(cx, "triste-pagamento-curto", 2);
    let (galeria, _fotos, _caixa) = ate_o_caixa(cx, &c, "Pagamento curto", 2, "100,00");
    c.b.teclar(cx, "f4");
    c.b.teclar(cx, "2");
    c.b.digitar(cx, "50,00");
    c.b.teclar(cx, "enter"); // lança 50 no PIX
    c.b.teclar(cx, "enter"); // tenta concluir com 30 faltando
    for _ in 0..8 {
        c.b.respirar(cx);
    }
    assert!(vendas_da_galeria(&c, &galeria).is_empty(), "não concluiu");
    c.b.caixa(cx, |caixa, _w, _cx| {
        assert_eq!(
            caixa.dialogo_do_caixa(),
            Some("Pagamento"),
            "continua no pagamento"
        )
    });
    let disse = erro_no_dialogo(&c, cx)
        .into_iter()
        .chain(avisos(&c, cx))
        .collect::<Vec<_>>();
    assert!(
        disse.iter().any(|t| t.contains("30,00")),
        "o balcão diz quanto falta: {disse:?}"
    );
    c.b.teclar(cx, "1"); // dinheiro, já com os 30,00 que faltam
    c.b.teclar(cx, "enter");
    c.b.teclar(cx, "enter");
    c.b.ate(cx, "a venda entra", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.ultima_venda_para_teste().is_some()
        })
    });
    let venda = vendas_da_galeria(&c, &galeria).remove(0);
    let formas: Vec<(String, i64)> = venda["pagamentos"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            (
                p["forma"].as_str().unwrap_or("").to_string(),
                p["valor_centavos"].as_i64().unwrap_or(0),
            )
        })
        .collect();
    assert_eq!(
        formas,
        vec![("pix".into(), 5000), ("dinheiro".into(), 3000)],
        "{venda}"
    );
}

/// 😕 **Valor inválido no pagamento**: o operador digita letras; o balcão recusa
/// com a frase do formato e não lança. Corrigido, a venda entra.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_valor_invalido_no_pagamento_e_recusado_e_corrigido(cx: &mut TestAppContext) {
    let c = preparar(cx, "triste-valor-invalido", 1);
    let (galeria, _fotos, _caixa) = ate_o_caixa(cx, &c, "Valor inválido", 1, "0,00");
    c.b.teclar(cx, "f4");
    c.b.teclar(cx, "2");
    c.b.digitar(cx, "abc");
    c.b.teclar(cx, "enter");
    assert!(
        erro_no_dialogo(&c, cx).is_some_and(|e| e.contains("formato")),
        "o balcão diz o formato: {:?}",
        erro_no_dialogo(&c, cx)
    );
    c.b.caixa(cx, |caixa, _w, _cx| {
        assert_eq!(caixa.pagamentos_lancados(), 0, "nada lançado")
    });
    c.b.teclar(cx, "cmd-a");
    c.b.digitar(cx, "40,00");
    c.b.teclar(cx, "enter");
    c.b.teclar(cx, "enter");
    c.b.ate(cx, "a venda entra depois de corrigir", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.ultima_venda_para_teste().is_some()
        })
    });
    assert_eq!(vendas_da_galeria(&c, &galeria)[0]["total_centavos"], 4000);
}

/// 😕 **F4 com o caixa fechado**: o balcão leva à abertura do caixa, e depois às
/// pessoas, antes do pagamento — a venda entra no caixa certo.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn vender_com_o_caixa_fechado_leva_a_abrir_e_depois_vende(cx: &mut TestAppContext) {
    let c = preparar(cx, "triste-caixa-fechado", 1);
    let (galeria, fotos) = criar_a_sessao(cx, &c, "Caixa fechado", "cliente-fechado@e2e.test");
    levar(cx, &c, &galeria, &fotos[..1]);
    c.b.ate(cx, "o caixa flutuante lê a sessão", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.sessao_escolhida() == Some(galeria.as_str()) && !caixa.carregando()
        })
    });
    c.b.teclar(cx, "f4");
    c.b.caixa(cx, |caixa, _w, _cx| {
        assert_eq!(
            caixa.dialogo_do_caixa(),
            Some("Abrir"),
            "F4 sem caixa leva a abrir o caixa"
        )
    });
    c.b.digitar(cx, "0,00");
    c.b.teclar(cx, "enter");
    c.b.ate(cx, "o caixa abre", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.caixa_do_estudio_aberto())
    });
    escolher_as_pessoas(cx, &c);
    pagar_em_pix(cx, &c);
    let caixa = c
        .site
        .get(&format!("/pos-venda/caixa?estudio_id={}", c.estudio_id));
    assert_eq!(
        caixa["vendas"].as_array().map_or(0, Vec::len),
        1,
        "a venda no caixa do estúdio: {caixa}"
    );
}

/// 😕 **Estorno acima do que foi pago**: o balcão recusa e nada sai do caixa;
/// com o valor certo, o estorno entra uma vez.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_estorno_acima_do_pago_e_recusado(cx: &mut TestAppContext) {
    let c = preparar(cx, "triste-estorno-alto", 2);
    let (galeria, fotos, caixa_id) = ate_o_caixa(cx, &c, "Estorno alto", 2, "100,00");
    pagar_em_pix(cx, &c);
    let venda = vendas_da_galeria(&c, &galeria).remove(0);
    let numero = venda["numero"].as_i64().unwrap_or_default();
    c.b.ate(cx, "a venda na lista do F7", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa
                .vendas_da_sessao_para_teste()
                .iter()
                .any(|(n, ..)| *n == numero)
        })
    });
    c.b.teclar(cx, "f7");
    let venda_id = venda["id"].as_str().expect("o id").to_string();
    c.b.clicar(cx, &format!("caixa-estornar-{venda_id}"));
    c.b.clicar(cx, &campo_do_caixa("Valor a devolver (até R$ 80,00)"));
    c.b.teclar(cx, "cmd-a");
    c.b.digitar(cx, "999,00");
    c.b.clicar(cx, "caixa-campo-motivo");
    c.b.digitar(cx, "o cliente desistiu");
    c.b.teclar(cx, "enter");
    for _ in 0..8 {
        c.b.respirar(cx);
    }
    assert!(
        erro_no_dialogo(&c, cx).is_some(),
        "o balcão recusa o estorno acima do pago"
    );
    assert_eq!(
        caixa_por_id(&c, &caixa_id)["estornos"]
            .as_array()
            .map_or(0, Vec::len),
        0,
        "nada saiu do caixa"
    );
    // Corrige o valor e confirma.
    c.b.clicar(cx, &campo_do_caixa("Valor a devolver (até R$ 80,00)"));
    c.b.teclar(cx, "cmd-a");
    c.b.digitar(cx, "40,00");
    c.b.teclar(cx, "enter");
    c.b.ate_na_api(cx, "o estorno certo entra", || {
        let n = caixa_por_id(&c, &caixa_id)["estornos"]
            .as_array()
            .map_or(0, Vec::len);
        (n == 1).then_some(()).ok_or(format!("{n}"))
    });
    let _ = fotos;
}

/// 😕 **Fechamento com a contagem errada**: a conferência mostra a diferença, o
/// operador corrige e fecha sem diferença — e a primeira contagem, às cegas,
/// fica guardada.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_fechamento_com_a_contagem_errada_e_corrigido(cx: &mut TestAppContext) {
    let c = preparar(cx, "triste-fechamento", 1);
    let (_galeria, _fotos, caixa_id) = ate_o_caixa(cx, &c, "Contagem errada", 1, "100,00");
    pagar_em_pix(cx, &c);
    // Contou 50 no dinheiro (eram 100) — e confere.
    pedir_o_fechamento_sem_fechar(cx, &c, "50,00", "40,00");
    c.b.clicar(cx, "caixa-corrigir");
    c.b.teclar(cx, "cmd-a");
    c.b.digitar(cx, "100,00");
    c.b.teclar(cx, "enter");
    c.b.ate(cx, "a contagem corrigida é conferida", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.contagem_conferida())
    });
    c.b.teclar(cx, "enter");
    c.b.ate(cx, "o caixa fecha", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.caixa_fechado_no_dialogo())
    });
    let caixa = caixa_por_id(&c, &caixa_id);
    assert_eq!(caixa["contado"]["dinheiro"], 10000, "{caixa}");
    assert_eq!(caixa["diferenca"]["dinheiro"], 0, "{caixa}");
    assert_eq!(
        caixa["primeira_contagem"]["dinheiro"], 5000,
        "a contagem às cegas fica guardada: {caixa}"
    );
}

/// 😕 **`B` em foto sem nota e em foto rejeitada**: o balcão avisa a regra de
/// cada uma, e nenhuma vai ao cupom.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_b_em_foto_sem_nota_ou_rejeitada_avisa_e_nao_vai_ao_cupom(cx: &mut TestAppContext) {
    let c = preparar(cx, "triste-b", 2);
    let (galeria, fotos) = criar_a_sessao(cx, &c, "B sem nota", "cliente-b@e2e.test");
    c.b.ir_na_grade(cx, &fotos[0]);
    c.b.teclar(cx, "b");
    c.b.ate(cx, "o balcão pede a nota", |b, cx| {
        b.detalhe(cx, |t, _w, _cx| {
            t.ultimo_aviso().is_some_and(|a| a.contains("lassifique"))
        })
    });
    c.b.ir_na_grade(cx, &fotos[1]);
    c.b.teclar(cx, "4");
    c.b.teclar(cx, "x");
    c.b.ate(cx, "a rejeitada sai do site", |b, cx| {
        b.detalhe(cx, |t, _w, _cx| t.como_esta(&fotos[1]).is_none_or(|f| f.2))
    });
    abrir_o_caixa(cx, &c, &galeria, "0,00");
    c.b.ate(cx, "nenhuma vai ao cupom", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.cupom_para_teste().1 == 0)
    });
    let no_site: Vec<Value> = fotos_no_painel(&c.site, &galeria);
    assert!(
        no_site.iter().all(|f| f["estado"] != "levada_no_balcao"),
        "nenhuma levada: {no_site:?}"
    );
}

/// 😕 **O cliente tenta comprar online a foto que já levou no balcão**: o site
/// recusa (409) — ela já é dele —, e a outra, à venda, ele compra.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_cliente_nao_compra_online_a_foto_que_ja_levou(cx: &mut TestAppContext) {
    let c = preparar(cx, "triste-compra-levada", 2);
    let (galeria, fotos) = criar_a_sessao(cx, &c, "Compra da levada", "cliente-levada@e2e.test");
    levar(cx, &c, &galeria, &fotos[..1]);
    c.b.ir_na_grade(cx, &fotos[1]);
    c.b.teclar(cx, "3");
    c.b.ate_na_api(cx, "a nota da outra chega", || {
        let f = foto_no_painel(&c.site, &galeria, &fotos[1]).ok_or("sumiu")?;
        (f["nota"] == 3).then_some(()).ok_or(format!("{f}"))
    });
    let cliente = o_cliente_entra_pelo_link(cx, &c);
    let (status, _) = cliente.post(
        &format!("/meus-ensaios/{galeria}/comprar"),
        json!({ "fotos": [fotos[0]] }),
    );
    assert_eq!(status, 409, "a levada já é do cliente");
    let (status, pedido) = cliente.post(
        &format!("/meus-ensaios/{galeria}/comprar"),
        json!({ "fotos": [fotos[1]] }),
    );
    assert_eq!(status, 201, "a à venda ele compra: {pedido}");
}
