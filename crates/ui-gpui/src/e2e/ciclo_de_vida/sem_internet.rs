//! 📴 **A internet cai no expediente** (dono, 20/out/2026: *"hoje ficamos sem
//! internet no estúdio… precisamos deixar o sistema offline first"*).
//!
//! O proxy de falhas derruba **todo** pedido do app à API, como o roteador do
//! estúdio sem link: a leitura vem do espelho, o gesto vai para a fila de
//! sincronização e a tela segue. Com a rede de volta, a fila leva tudo ao site,
//! na ordem, e a API termina igual à tela — sem duplicar nada.

use gpui_kit::TestAppContext;
use serde_json::json;

use super::*;

/// Todo pedido do app à API cai, até o teste limpar.
fn derrubar_a_internet(c: &Cena) {
    c.falhas
        .programar(json!({ "metodo": "", "caminho": "/", "modo": "cair", "vezes": 0 }));
}

/// O selo do rodapé, como a raiz o vê.
fn fila(cx: &mut TestAppContext, c: &Cena) -> crate::sincronizacao::Situacao {
    c.b.app(cx, |app, _w, _cx| app.situacao_da_fila)
}

/// A rede volta, e o operador pede "Tentar agora" no selo do rodapé — a fila
/// não espera o fim da pausa dela.
fn a_internet_volta(cx: &mut TestAppContext, c: &Cena) {
    c.falhas.limpar();
    // O selo acende pela vigia da raiz, que olha a fila a cada segundo.
    c.b.ate(cx, "o selo da fila aparece no rodapé", |b, cx| {
        b.desenhado(cx, "rodape-fila")
    });
    c.b.clicar(cx, "rodape-fila");
    c.b.ate(cx, "a lista da fila abre", |b, cx| {
        b.desenhado(cx, "lista-da-fila")
    });
    c.b.clicar(cx, "fila-tentar-agora");
}

/// 🎬 O dia sem internet: a sessão do dia já estava aberta; a rede cai; o
/// operador classifica, leva no balcão, abre o caixa e vende em PIX. A tela
/// mostra tudo e o selo conta o que espera. A rede volta: as marcas, a abertura
/// e a venda chegam ao site, uma vez cada.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn a_internet_cai_no_expediente_e_tudo_sobe_quando_volta(cx: &mut TestAppContext) {
    let c = preparar(cx, "sem-internet", 3);
    let (galeria, fotos) =
        criar_a_sessao(cx, &c, "Sem internet no balcão", "cliente-offline@e2e.test");
    // O caixa flutuante leu a sessão com rede: o espelho tem o que ele lê.
    c.b.ate(cx, "o caixa flutuante lê a sessão", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.sessao_escolhida() == Some(galeria.as_str()) && !caixa.carregando()
        })
    });

    derrubar_a_internet(&c);

    // ⭐🛍️ A classificação continua: nota e `B`, e a grade não desfaz nada.
    for foto in &fotos[..2] {
        c.b.ir_na_grade(cx, foto);
        c.b.teclar(cx, "5");
        c.b.teclar(cx, "b");
    }
    c.b.ate(cx, "a grade guarda a nota e a levada sem rede", |b, cx| {
        b.detalhe(cx, |t, _w, _cx| {
            fotos[..2].iter().all(|f| {
                t.como_esta(f).is_some_and(|(estado, nota, _)| {
                    nota == Some(5) && estado == Estado::LevadaNoBalcao
                })
            })
        })
    });
    c.b.ate(
        cx,
        "o selo diz sem internet e conta o que espera",
        |b, cx| {
            b.app(cx, |app, _w, _cx| {
                app.situacao_da_fila.sem_rede_desde.is_some() && app.situacao_da_fila.pendentes > 0
            })
        },
    );
    assert!(
        fotos_no_painel(&c.site, &galeria)
            .iter()
            .all(|f| f["estado"] != "levada_no_balcao"),
        "nada chegou ao site ainda"
    );

    // 🧾 O caixa abre sem rede, e a venda entra.
    c.b.teclar(cx, "f8");
    c.b.caixa(cx, |caixa, _w, _cx| {
        assert_eq!(caixa.dialogo_do_caixa(), Some("Abrir"), "F8 abre o caixa")
    });
    c.b.digitar(cx, "0,00");
    c.b.teclar(cx, "enter");
    c.b.ate(cx, "o caixa aparece aberto sem rede", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.caixa_do_estudio_aberto())
    });
    c.b.ate(cx, "o cupom tem as duas levadas", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.cupom_para_teste() == (fotos[..2].to_vec(), 8000)
        })
    });
    escolher_as_pessoas(cx, &c);
    pagar_em_pix(cx, &c);
    assert!(
        vendas_da_galeria(&c, &galeria).is_empty(),
        "a venda ainda não chegou ao site"
    );
    let antes_da_volta = fila(cx, &c).pendentes;
    assert!(
        antes_da_volta >= 4,
        "marcas, abertura e venda esperam: {antes_da_volta}"
    );

    // 📶 A internet volta.
    a_internet_volta(cx, &c);
    c.b.ate_na_api(cx, "as levadas chegam ao site", || {
        let levadas: Vec<String> = fotos_no_painel(&c.site, &galeria)
            .into_iter()
            .filter(|f| f["estado"] == "levada_no_balcao" && f["nota"] == 5)
            .filter_map(|f| f["id"].as_str().map(str::to_string))
            .collect();
        (levadas.len() == 2)
            .then_some(())
            .ok_or(format!("{levadas:?}"))
    });
    c.b.ate_na_api(cx, "a venda chega uma vez, pelo preço cheio", || {
        let vendas = vendas_da_galeria(&c, &galeria);
        match vendas.as_slice() {
            [venda] if venda["total_centavos"] == 8000 => Ok(()),
            outras => Err(format!("{outras:?}")),
        }
    });
    let aberto = c
        .site
        .get(&format!("/pos-venda/caixa?estudio_id={}", c.estudio_id));
    assert!(aberto["id"].is_string(), "o caixa abriu no site: {aberto}");
    c.b.ate(cx, "a fila esvazia e o selo apaga", |b, cx| {
        b.app(cx, |app, _w, _cx| {
            app.situacao_da_fila.pendentes == 0 && app.situacao_da_fila.sem_rede_desde.is_none()
        })
    });
    assert_eq!(fila(cx, &c).conflitos, 0, "nada foi recusado na volta");
    c.b.ato_limpo(cx, "o dia sem internet");
}

/// 💥 Sem rede, o app fecha de supetão com a fila cheia — e reabre ainda sem
/// rede: a nota continua na grade. A rede volta e ela chega ao site.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn sem_rede_o_app_fecha_e_a_fila_sobrevive(cx: &mut TestAppContext) {
    let mut c = preparar(cx, "sem-internet-reabrir", 1);
    let (galeria, fotos) = criar_a_sessao(cx, &c, "Fila que sobrevive", "cliente-fila@e2e.test");
    derrubar_a_internet(&c);
    c.b.ir_na_grade(cx, &fotos[0]);
    c.b.teclar(cx, "4");
    c.b.ate(cx, "a nota fica guardada", |b, cx| {
        b.app(cx, |app, _w, _cx| app.situacao_da_fila.pendentes > 0)
    });

    fechar_de_supetao_e_reabrir(cx, &mut c);
    derrubar_a_internet(&c);
    assert!(
        fotos_no_painel(&c.site, &galeria)[0]["nota"].is_null(),
        "o site ainda não tem a nota"
    );
    a_internet_volta(cx, &c);
    c.b.ate_na_api(cx, "a nota de antes de fechar chega ao site", || {
        let nota = fotos_no_painel(&c.site, &galeria)[0]["nota"].clone();
        (nota == 4).then_some(()).ok_or(format!("nota: {nota}"))
    });
}
