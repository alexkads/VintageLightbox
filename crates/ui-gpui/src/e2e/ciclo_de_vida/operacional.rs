//! 🌪️ **O fluxo operacional sob falha** — o cliente na frente, as fotos
//! subindo, o operador classificando e levando, e a rede oscilando (dono,
//! 03/out/2026: *"é durante o fluxo operacional que mora o perigo"*).
//!
//! A régua: nenhuma foto se perde nem aparece em dobro, nenhum gesto do
//! operador some, e o cupom cobra exatamente o que o cliente levou.

use super::*;

/// 🌪️ **A subida das fotos com a rede oscilando**: duas respostas perdidas
/// (a API gravou a foto) e seis quedas antes de chegar — mais do que as três
/// tentativas antigas aguentavam. No fim, as seis estão
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
                "metodo": "POST", "caminho": "/fotos", "modo": "cair", "vezes": 6
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

/// 🌪️ **O cliente está pagando online a foto que o balcão quer levar.** Ele
/// abre o link e põe a foto no pedido (sem pagar ainda); o operador aperta `B`
/// nela. O balcão avisa e não a cobra — senão o cliente pagaria duas vezes — e,
/// aprovado o pagamento online, a foto aparece comprada no balcão sozinha.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_cliente_pagando_online_a_foto_que_o_balcao_quer_levar(cx: &mut TestAppContext) {
    let c = preparar(cx, "operacional-disputa", 2);
    let (galeria, fotos) = criar_a_sessao(cx, &c, "Disputa da foto", "cliente-disputa@e2e.test");
    let disputada = fotos[0].clone();
    c.b.ir_na_grade(cx, &disputada);
    c.b.teclar(cx, "5");
    c.b.ate_na_api(cx, "a nota chega", || {
        let f = foto_no_painel(&c.site, &galeria, &disputada).ok_or("sumiu")?;
        (f["nota"] == 5).then_some(()).ok_or(format!("{f}"))
    });
    let cliente = o_cliente_entra_pelo_link(cx, &c);
    let (status, pedido) = cliente.post(
        &format!("/meus-ensaios/{galeria}/comprar"),
        json!({ "fotos": [disputada] }),
    );
    assert_eq!(status, 201, "o cliente põe a foto no pedido: {pedido}");
    let pedido_id = pedido["order_id"].as_str().expect("o pedido").to_string();

    // O operador tenta levar a mesma foto: o balcão avisa e ela fica à venda.
    c.b.ir_na_grade(cx, &disputada);
    c.b.teclar(cx, "b");
    c.b.ate(
        cx,
        "o balcão avisa que o cliente está pagando no site",
        |b, cx| {
            b.app(cx, |app, _w, _cx| {
                app.avisos_dados_para_teste()
                    .iter()
                    .any(|(texto, erro)| *erro && texto.contains("pagando"))
            })
        },
    );
    let no_site = foto_no_painel(&c.site, &galeria, &disputada).expect("a foto");
    assert_eq!(no_site["estado"], "disponivel", "não foi levada: {no_site}");
    abrir_o_caixa(cx, &c, &galeria, "0,00");
    c.b.ate(cx, "o cupom não a cobra", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.cupom_para_teste().1 == 0)
    });

    // O pagamento online é aprovado: a foto é do pedido, e o balcão vê sozinho.
    aprovar_o_pedido(&c, &pedido_id);
    c.b.ate(cx, "o balcão mostra a foto comprada online", |b, cx| {
        b.detalhe(cx, |tela, _w, _cx| {
            tela.como_esta(&disputada).map(|f| f.0) == Some(Estado::Comprada)
        })
    });
    assert!(
        vendas_da_galeria(&c, &galeria).is_empty(),
        "nada foi cobrado no balcão"
    );
}

/// 🌪️ **Vender com as fotos ainda subindo devagar** (cada envio leva 3 s): o
/// operador dá nota e leva enquanto elas sobem, abre o caixa, e o cupom cobra as
/// três assim que chegam.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn vender_com_as_fotos_ainda_subindo(cx: &mut TestAppContext) {
    let c = preparar(cx, "operacional-subindo", 3);
    let quantas = 3;
    let b = &c.b;
    b.clicar(cx, "sessoes-nova");
    b.ate(cx, "o assistente abre", |b, cx| {
        b.app(cx, |app, _w, _cx| app.tela() == Tela::NovaSessao)
    });
    b.clicar(cx, "nova-escolher-fotos");
    b.ate(cx, "as fotos entram no rascunho", |b, cx| {
        b.app(cx, |app, _w, cx| {
            app.nova_sessao.read(cx).quantas_fotos() == quantas
        })
    });
    b.clicar(cx, "nova-avancar");
    b.clicar(cx, "nova-titulo");
    b.digitar(cx, "Vender subindo");
    b.clicar(cx, "nova-email");
    b.digitar(cx, "cliente-subindo@e2e.test");
    let formulario = |b: &Balcao, cx: &mut TestAppContext| {
        b.app(cx, |app, _w, cx| {
            app.nova_sessao.read(cx).rascunho_para_teste().formulario
        })
    };
    b.escolher_na_lista(cx, "nova-produto", &c.produto_id, |b, cx| {
        formulario(b, cx).produto_id
    });
    b.escolher_na_lista(cx, "nova-estudio", &c.estudio_id, |b, cx| {
        formulario(b, cx).estudio_id
    });
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/fotos", "modo": "atrasar", "ms": 3000, "vezes": 0
    }));
    b.clicar(cx, "nova-criar-cabecalho");
    b.ate(cx, "a sessão é criada", |b, cx| {
        b.app(cx, |app, _w, cx| {
            app.tela() == Tela::Sessao && app.detalhe.read(cx).galeria_id().is_some()
        })
    });
    let galeria = b
        .detalhe(cx, |t, _w, _cx| t.galeria_id().map(str::to_string))
        .expect("a galeria");
    // As fotos ainda estão subindo: o operador anda pela grade e marca.
    b.ate(cx, "as três aparecem na grade, subindo", |b, cx| {
        b.detalhe(cx, |t, _w, _cx| t.ids_visiveis().len() == 3)
    });
    let na_grade = b.detalhe(cx, |t, _w, _cx| t.ids_visiveis());
    for id in &na_grade {
        b.ir_na_grade(cx, id);
        b.teclar(cx, "5");
        b.teclar(cx, "b");
    }
    abrir_o_caixa(cx, &c, &galeria, "0,00");
    b.ate(cx, "o cupom cobra as três quando chegam", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.cupom_para_teste().1 == 12000)
    });
    c.falhas.limpar();
    escolher_as_pessoas(cx, &c);
    pagar_em_pix(cx, &c);
    let vendas = vendas_da_galeria(&c, &galeria);
    assert_eq!(vendas.len(), 1);
    assert_eq!(
        vendas[0]["total_centavos"], 12000,
        "as três levadas cobradas"
    );
    b.ato_limpo(cx, "vender com as fotos subindo");
}

/// 💥 **O app fecha de supetão no meio da subida.** A rede está caída, as fotos
/// não sobem, o operador já deu nota e levou duas — marcas que só existem no
/// disco —, e o app some. A rede volta, o app reabre no mesmo catálogo: as
/// quatro sobem, nenhuma em dobro, e as duas marcas chegam ao site.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_app_fecha_no_meio_da_subida_e_ao_reabrir_nada_se_perde(cx: &mut TestAppContext) {
    let mut c = preparar(cx, "operacional-reabrir", 4);
    let galeria = iniciar_a_sessao(
        cx,
        &c,
        "App fechado no meio",
        "cliente-reabrir@e2e.test",
        &|| {
            c.falhas.programar(json!({
                "metodo": "POST", "caminho": "/fotos", "modo": "cair", "vezes": 0
            }));
        },
    );
    c.b.ate(cx, "as quatro aparecem na grade, sem subir", |b, cx| {
        b.detalhe(cx, |t, _w, _cx| t.ids_visiveis().len() == 4)
    });
    let marcadas: Vec<String> = c.b.detalhe(cx, |t, _w, _cx| t.ids_visiveis())[..2].to_vec();
    for id in &marcadas {
        c.b.ir_na_grade(cx, id);
        c.b.teclar(cx, "5");
        c.b.teclar(cx, "b");
    }
    assert!(
        fotos_no_painel(&c.site, &galeria).is_empty(),
        "nada subiu ainda"
    );

    fechar_de_supetao_e_reabrir(cx, &mut c);
    c.falhas.limpar();
    // A retomada leva de volta à sessão (0.1.79); se não levar, o operador a abre.
    let voltou = {
        let ate = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            c.b.respirar(cx);
            if c.b.app(cx, |app, _w, _cx| app.tela() == Tela::Sessao) {
                break true;
            }
            if std::time::Instant::now() > ate {
                break false;
            }
        }
    };
    if !voltou {
        // Depois de um fechamento brusco não há onde voltar: o operador abre a
        // sessão pela linha da lista.
        // 🚨 A linha, e não só a tela: a lista chega da API depois de a tela
        // abrir, e o clique cedo demais achava a lista vazia (04/out/2026, na
        // rodada do `make producao` com a máquina compilando do zero).
        let linha = format!("sessao-{galeria}");
        c.b.ate(cx, "a sessão aparece na lista", |b, cx| {
            b.app(cx, |app, _w, _cx| app.tela() == Tela::Sessoes) && b.desenhado(cx, &linha)
        });
        c.b.clicar(cx, &linha);
    }
    let fotos = conferir_a_sessao_criada(
        cx,
        &c,
        &galeria,
        "App fechado no meio",
        "cliente-reabrir@e2e.test",
    );
    assert_eq!(fotos.len(), 4);
    c.b.ate_na_api(
        cx,
        "as duas marcas feitas antes de fechar chegam ao site",
        || {
            let levadas: Vec<(Option<i64>, String)> = fotos_no_painel(&c.site, &galeria)
                .iter()
                .filter(|f| f["estado"] == "levada_no_balcao")
                .map(|f| {
                    (
                        f["nota"].as_i64(),
                        f["arquivo"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect();
            (levadas.len() == 2 && levadas.iter().all(|(n, _)| *n == Some(5)))
                .then_some(())
                .ok_or(format!("{levadas:?}"))
        },
    );
}

/// 🌪️ **Salvar a revelação com a rede caindo**: o operador clica em "Salvar na
/// galeria" e volta ao cliente; as cinco primeiras idas da revelação ao site
/// caem (a política antiga desistia na terceira). Ela chega sozinha depois — sem o operador refazer — e o bruto fica.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn salvar_a_revelacao_com_a_rede_caindo_chega_depois(cx: &mut TestAppContext) {
    let c = preparar(cx, "operacional-revelacao", 1);
    let (galeria, fotos) = criar_a_sessao(
        cx,
        &c,
        "Revelação com a rede caindo",
        "cliente-revela@e2e.test",
    );
    let foto = fotos[0].clone();
    let (_, bruto_antes) = c.site.bytes(&format!("/pos-venda/fotos/{foto}/original"));
    c.b.ir_na_grade(cx, &foto);
    c.b.clicar(cx, "sessao-revelar");
    c.b.ate(cx, "a Revelação abre com a foto", |b, cx| {
        b.revelacao(cx, |tela, _w, _cx| tela.tem_pixels())
    });
    let predefinicao = c.b.revelacao(cx, |tela, _w, cx| {
        tela.coluna_de_predefinicoes(cx)
            .0
            .first()
            .cloned()
            .expect("há predefinições")
    });
    c.b.clicar(cx, &format!("predefinicao-{predefinicao}"));
    c.b.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.6, cx));
    c.falhas.programar(json!({
        "metodo": "POST", "caminho": "/bilhete-de-revelacao", "modo": "cair", "vezes": 5
    }));
    c.b.clicar(cx, "revelacao-salvar-na-galeria");
    c.b.ate(cx, "o Salvar devolve o operador à sessão", |b, cx| {
        b.app(cx, |app, _w, _cx| app.tela() == Tela::Sessao)
    });
    c.b.ate_na_api(cx, "a revelação chega ao site depois das quedas", || {
        let f = foto_no_painel(&c.site, &galeria, &foto).ok_or("sumiu")?;
        let exposicao = f["ajustes"]["exposure"].as_f64().unwrap_or(f64::NAN);
        (!f["revelada_em"].is_null() && (exposicao - 0.6).abs() < 1e-3)
            .then_some(())
            .ok_or(format!("ainda não: {}", f["ajustes"]))
    });
    c.b.ate(cx, "a fila de envios esvazia", |b, cx| {
        b.app(cx, |app, _w, _cx| app.a_subir_para_teste().is_empty())
    });
    let (_, bruto_depois) = c.site.bytes(&format!("/pos-venda/fotos/{foto}/original"));
    assert!(bruto_antes == bruto_depois, "o bruto nunca muda");
    c.b.ato_limpo(cx, "a revelação com a rede caindo");
}
