//! 🌪️ **O fluxo operacional sob falha** — o cliente na frente, as fotos
//! subindo, o operador classificando e levando, e a rede oscilando (dono,
//! 03/out/2026: *"é durante o fluxo operacional que mora o perigo"*).
//!
//! A régua: nenhuma foto se perde nem aparece em dobro, nenhum gesto do
//! operador some, e o cupom cobra exatamente o que o cliente levou.

use super::*;

/// 🌪️ **A subida das fotos com a rede oscilando**: duas respostas perdidas
/// (a API gravou a foto) e duas quedas antes de chegar. No fim, as seis estão
/// no site — nenhuma perdida, nenhuma em dobro — e a fila do balcão esvazia.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_rede_oscila_na_subida_e_nenhuma_foto_se_perde_nem_duplica(cx: &mut TestAppContext) {
    let c = preparar(cx, "operacional-subida", 6);
    let (galeria, fotos) = criar_a_sessao_com(
        cx,
        &c,
        "Subida com a rede oscilando",
        "cliente-subida@e2e.test",
        &|| {
            c.falhas.programar(json!({
                "metodo": "POST", "caminho": "/fotos", "modo": "engolir", "vezes": 2
            }));
            c.falhas.programar(json!({
                "metodo": "POST", "caminho": "/fotos", "modo": "cair", "vezes": 2
            }));
        },
    );
    // `criar_a_sessao` já esperou as seis no site; e nenhuma a mais.
    let todas = c.site.get(&format!("/pos-venda/galerias/{galeria}"))["fotos"].clone();
    let arquivos: Vec<String> = todas
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["apagada_em"].is_null())
        .filter_map(|f| f["arquivo"].as_str().map(str::to_string))
        .collect();
    let mut sem_repetir = arquivos.clone();
    sem_repetir.sort();
    sem_repetir.dedup();
    assert_eq!(
        (arquivos.len(), sem_repetir.len()),
        (6, 6),
        "seis fotos, nenhuma em dobro: {arquivos:?}"
    );
    assert_eq!(fotos.len(), 6);
    c.b.ate(cx, "a fila de envios do balcão esvazia", |b, cx| {
        b.app(cx, |app, _w, _cx| app.a_subir_para_teste().is_empty())
    });
    c.b.ato_limpo(cx, "a subida com a rede oscilando");
}

/// 🌪️ **A rede cai enquanto o operador classifica e leva**: três gestos
/// (nota e `B`) batem na rede caída. No fim, o site sabe de todos, e o cupom
/// cobra exatamente as levadas.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_rede_cai_na_classificacao_e_o_cupom_cobra_certo(cx: &mut TestAppContext) {
    let c = preparar(cx, "operacional-classificacao", 4);
    let (galeria, fotos) = criar_a_sessao(
        cx,
        &c,
        "Classificação com a rede caindo",
        "cliente-classifica@e2e.test",
    );
    c.falhas.programar(json!({
        "metodo": "PATCH", "caminho": "/pos-venda/fotos/", "modo": "cair", "vezes": 3
    }));
    levar(cx, &c, &galeria, &fotos[..3]);
    abrir_o_caixa(cx, &c, &galeria, "0,00");
    c.b.ate(cx, "o cupom tem as três levadas, pelo cheio", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.cupom_para_teste() == (fotos[..3].to_vec(), 12000)
        })
    });
    c.b.ato_limpo(cx, "a classificação com a rede caindo");
}

/// 🌪️ **API lenta, operador rápido**: cada gesto demora 1,2 s para responder,
/// e o operador dá nota e leva quatro fotos sem esperar. Nenhum gesto se perde.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_api_lenta_com_o_operador_rapido_nao_perde_gesto(cx: &mut TestAppContext) {
    let c = preparar(cx, "operacional-lento", 5);
    let (galeria, fotos) = criar_a_sessao(cx, &c, "API lenta", "cliente-lento@e2e.test");
    c.falhas.programar(json!({
        "metodo": "PATCH", "caminho": "/pos-venda/fotos/", "modo": "atrasar", "ms": 1200, "vezes": 0
    }));
    let notas = ["5", "4", "3", "2"];
    for (foto, nota) in fotos[..4].iter().zip(notas) {
        c.b.ir_na_grade(cx, foto);
        c.b.teclar(cx, nota);
        c.b.teclar(cx, "b");
    }
    c.b.ate_na_api(cx, "o site tem cada nota e cada levada", || {
        let no_site = fotos_no_painel(&c.site, &galeria);
        let lido: Vec<(Option<i64>, String)> = fotos[..4]
            .iter()
            .map(|id| {
                let f = no_site
                    .iter()
                    .find(|f| f["id"] == id.as_str())
                    .cloned()
                    .unwrap_or(Value::Null);
                (
                    f["nota"].as_i64(),
                    f["estado"].as_str().unwrap_or("").to_string(),
                )
            })
            .collect();
        let esperado: Vec<(Option<i64>, String)> = [5, 4, 3, 2]
            .iter()
            .map(|n| (Some(*n), "levada_no_balcao".to_string()))
            .collect();
        (lido == esperado).then_some(()).ok_or(format!("{lido:?}"))
    });
    c.falhas.limpar();
    c.b.ato_limpo(cx, "a API lenta com o operador rápido");
}

/// 🌪️ **A rede some de vez durante a classificação**: o balcão tenta por
/// alguns segundos, desiste, **avisa e desfaz** na grade — não mente que a
/// foto foi levada. Com a rede de volta, o operador refaz e o cupom cobra certo.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_rede_some_na_classificacao_e_o_balcao_avisa_sem_mentir(cx: &mut TestAppContext) {
    let c = preparar(cx, "operacional-sem-rede", 2);
    let (galeria, fotos) = criar_a_sessao(
        cx,
        &c,
        "Sem rede na classificação",
        "cliente-semrede@e2e.test",
    );
    c.falhas.programar(json!({
        "metodo": "PATCH", "caminho": "/pos-venda/fotos/", "modo": "cair", "vezes": 0
    }));
    c.b.ir_na_grade(cx, &fotos[0]);
    c.b.teclar(cx, "5");
    c.b.ate(cx, "o balcão desiste, avisa e desfaz a nota", |b, cx| {
        b.app(cx, |app, _w, cx| {
            let nota = app.detalhe.read(cx).como_esta(&fotos[0]).and_then(|c| c.1);
            nota.is_none() && app.avisos_dados_para_teste().iter().any(|(_, erro)| *erro)
        })
    });
    let no_site = foto_no_painel(&c.site, &galeria, &fotos[0]).expect("a foto");
    assert!(
        no_site["nota"].is_null(),
        "o site não tem a nota: {no_site}"
    );

    // A rede volta; o operador refaz.
    c.falhas.limpar();
    levar(cx, &c, &galeria, &fotos[..1]);
    abrir_o_caixa(cx, &c, &galeria, "0,00");
    c.b.ate(cx, "o cupom cobra a levada", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.cupom_para_teste() == (fotos[..1].to_vec(), 4000)
        })
    });
}
