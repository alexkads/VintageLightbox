//! 🎬 **O atendimento inteiro, com a foto no centro de cada asserção.**
//!
//! Os outros cenários percorrem o fluxo; estes **olham para as fotos** em cada
//! parada — quais estão na grade, em que ordem, com que estado e nota, quais
//! sobram em cada recorte, qual entra na revelação, qual a tira mostra e qual
//! chega à tela do cliente.
//!
//! # Por que contar não basta
//!
//! 🚨 **Um número não diz qual foto.** Um recorte que deixasse a comprada
//! entrar em "à venda", uma tira que perdesse a ordem da grade, uma revelação
//! que abrisse a vizinha — todos passam num teste que só soma. É por isso que
//! aqui se afirma a **lista de ids**, e não o tamanho dela.
//!
//! # O que cada cenário cobre
//!
//! | Cenário | O que ele prende |
//! |---|---|
//! | [`a_grade_mostra_as_fotos_certas_em_cada_recorte`] | a lista de ids por recorte, a foto local sem nota, e o que o painel diz da foto em foco |
//! | [`classificar_sinalizar_e_o_painel_acompanham_a_foto`] | nota, `P`, faixa, preço de venda e apagar — cada gesto conferido **na foto**, e o que sobe ao site |
//! | [`a_tira_da_revelacao_segue_o_recorte_da_grade`] | a revelação abre na foto certa, a tira tem as do recorte na ordem, e andar troca a foto aberta |
//! | [`a_tela_do_cliente_mostra_a_foto_da_vez_em_cada_tela`] | o cliente acompanha grade → revelação → gesto → volta, sempre com a foto certa |

use gpui::TestAppContext;

use super::{abrir_o_ensaio, Cenario};
use crate::app::Tela;
use crate::sessoes::detalhe::Pedido;
use biblioteca_core::acervo::{Estado, Filtro};

/// Os ids visíveis na grade da sessão, na ordem — a mesma da tira.
fn na_grade(e: &super::Estudio, cx: &mut TestAppContext) -> Vec<String> {
    e.detalhe(cx, |tela, _w, _cx| tela.ids_visiveis())
}

/// `(estado, nota, revelada)` de uma foto — o que a célula desenha.
fn como_esta(
    e: &super::Estudio,
    cx: &mut TestAppContext,
    id: &str,
) -> Option<(Estado, Option<u8>, bool)> {
    e.detalhe(cx, |tela, _w, _cx| tela.como_esta(id))
}

fn recortar(e: &super::Estudio, cx: &mut TestAppContext, filtro: Filtro) {
    e.detalhe(cx, |tela, _w, cx| tela.filtrar(filtro, cx));
}

/// 📸 **Cada recorte mostra as fotos certas, e não só a conta certa.**
///
/// O cenário padrão tem quatro do site — duas levadas (`a`, `b`), uma à venda
/// (`d`) e uma comprada (`c`) — e duas locais, que ainda não subiram e por isso
/// entram sem nota.
#[gpui::test]
fn a_grade_mostra_as_fotos_certas_em_cada_recorte(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    // A grade é uma só: as do site e as do disco, na ordem em que a sessão as
    // devolve — as locais por último, porque entram depois.
    assert_eq!(
        na_grade(&e, cx),
        ["a", "b", "d", "c", "id-DSC_101.jpg", "id-DSC_102.jpg"],
        "a grade da sessão mistura o site e o disco, sem reordenar"
    );

    // 🚨 A foto local entra **sem nota**: o que a leva ao site é a
    // classificação, e até lá ela não pode estar levada nem à venda.
    assert_eq!(
        como_esta(&e, cx, "id-DSC_101.jpg"),
        Some((Estado::Disponivel, None, false))
    );
    assert_eq!(
        como_esta(&e, cx, "a"),
        Some((Estado::LevadaNoBalcao, Some(4), false))
    );
    assert_eq!(
        como_esta(&e, cx, "c"),
        Some((Estado::Comprada, Some(5), false))
    );

    // Classificadas: as quatro do site têm nota; as duas locais, não.
    recortar(&e, cx, Filtro::Classificadas);
    assert_eq!(na_grade(&e, cx), ["a", "b", "d", "c"]);

    // Sinalizadas (levadas no balcão): só as duas.
    recortar(&e, cx, Filtro::Situacao(Estado::LevadaNoBalcao));
    assert_eq!(na_grade(&e, cx), ["a", "b"]);

    // À venda: a comprada **não** entra — ela já foi paga, e oferecer de novo é
    // o erro que a coluna de estado existe para impedir.
    recortar(&e, cx, Filtro::Situacao(Estado::Disponivel));
    assert_eq!(na_grade(&e, cx), ["d"]);

    // Compradas: só a `c`.
    recortar(&e, cx, Filtro::Situacao(Estado::Comprada));
    assert_eq!(na_grade(&e, cx), ["c"]);

    // Sem nota: as duas locais, que é o recorte "para esvaziar".
    recortar(&e, cx, Filtro::SemNota);
    assert_eq!(na_grade(&e, cx), ["id-DSC_101.jpg", "id-DSC_102.jpg"]);

    // 🔑 **Trocar o recorte limpa a seleção** — a mesma regra do site: o que se
    // vê é o que se opera, e as posições passam a apontar outras fotos.
    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("id-DSC_101.jpg", cx);
        tela.selecionar_tudo(cx);
        assert_eq!(tela.quantas_marcadas(), 2);
        tela.filtrar(Filtro::Todas, cx);
        assert_eq!(
            tela.quantas_marcadas(),
            0,
            "o recorte novo não herda marcação"
        );
    });
}

/// ⭐ **Classificar, sinalizar e mexer no painel — conferido na foto.**
///
/// A nota de uma local a leva ao site (passo 3); o `P` alterna o estado da do
/// site; a faixa e o preço de venda vão num `PATCH` com só o que mudou; apagar
/// pergunta antes e some com ela.
#[gpui::test]
fn classificar_sinalizar_e_o_painel_acompanham_a_foto(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    // ⭐ A nota numa foto **local**: ela sobe, com a nota que acabou de ser dada.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("id-DSC_101.jpg", cx));
    e.teclar(cx, "4");
    e.esperar(cx);
    let subidas = e.site.subidas();
    assert_eq!(subidas.len(), 1, "classificar sobe: {subidas:?}");
    assert_eq!(subidas[0].1, "id-DSC_101.jpg");
    assert_eq!(e.site.notas_pedidas(), vec![Some(4)]);

    // 🚩 O `P` numa foto do site alterna levada ↔ à venda, e a grade mostra.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "p");
    e.esperar(cx);
    assert_eq!(
        e.site
            .negociadas()
            .last()
            .map(|(id, m)| (id.clone(), m.estado)),
        Some((
            "d".to_string(),
            Some(domain::services::pos_venda::EstadoNoBalcao::LevadaNoBalcao)
        )),
        "o P manda o estado novo para a foto certa"
    );

    // 🚨 A comprada não muda: o site recusaria, e a tela nem tenta.
    let antes = e.site.negociadas().len();
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("c", cx));
    e.teclar(cx, "p");
    e.esperar(cx);
    assert_eq!(
        e.site.negociadas().len(),
        antes,
        "comprada não vai ao site pelo P"
    );

    // 🧾 A faixa e o preço de venda, pelo painel.
    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("d", cx);
        tela.mudar_faixa_para_teste("p1", cx);
    });
    e.esperar(cx);
    assert_eq!(
        e.site
            .negociadas()
            .last()
            .map(|(id, m)| (id.clone(), m.produto_id.clone())),
        Some(("d".to_string(), Some(Some("p1".to_string())))),
        "a faixa da foto vai no PATCH"
    );

    // 🗑️ Apagar pergunta antes — e o nome na pergunta é o da foto em foco.
    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("d", cx);
        tela.apagar_do_site_para_teste(cx);
        assert_eq!(
            tela.arquivo_na_pergunta_de_apagar().as_deref(),
            Some("d.jpg"),
            "a pergunta diz qual foto sai"
        );
        tela.cancelar_apagar(cx);
    });
    e.esperar(cx);
    assert!(
        e.site.tiradas().is_empty(),
        "cancelar não apaga foto nenhuma"
    );

    e.detalhe(cx, |tela, _w, cx| {
        tela.apagar_do_site_para_teste(cx);
        tela.confirmar_apagar(cx);
    });
    e.esperar(cx);
    assert_eq!(e.site.tiradas(), ["d"], "confirmar apaga a foto em foco");
}

/// 🎞️ **A tira da revelação é o recorte da grade, na mesma ordem.**
///
/// Entrar pela foto em foco abre **ela**; a tira traz as do recorte; as setas
/// andam por elas, e a foto aberta acompanha.
#[gpui::test]
fn a_tira_da_revelacao_segue_o_recorte_da_grade(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    // Com o recorte "Sinalizadas", a tira tem `a` e `b` — e só elas.
    recortar(&e, cx, Filtro::Situacao(Estado::LevadaNoBalcao));
    assert_eq!(na_grade(&e, cx), ["a", "b"]);

    e.revelar_a_do_site(cx, "b");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Revelacao));
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.foto_aberta().map(|f| f.name.clone()),
            Some("b.jpg".into()),
            "abre na foto que estava em foco"
        );
        assert_eq!(
            tela.ids_na_tira(),
            ["a", "b"],
            "a tira é o recorte da grade, na ordem dela"
        );
    });

    // ← anda para a anterior da tira.
    e.teclar(cx, "left");
    e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.foto_aberta().map(|f| f.name.clone()),
            Some("a.jpg".into()),
            "a seta troca a foto aberta"
        );
    });

    // Um gesto grava na foto aberta, e não na vizinha.
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 1.5, cx));
    e.esperar(cx);
    let gravadas = e.gravador.gravado();
    assert!(
        gravadas.iter().all(|(id, _, _)| id == "site:a"),
        "só a foto aberta foi gravada: {gravadas:?}"
    );
}

/// 🖥️ **A tela do cliente mostra a foto da vez, em cada tela.**
///
/// É o que o cliente vê do outro lado do balcão: a foto em foco na grade, a
/// aberta na revelação, o gesto ao vivo — e nunca a de antes.
#[gpui::test]
fn a_tela_do_cliente_mostra_a_foto_da_vez_em_cada_tela(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    let no_cliente = |cx: &mut TestAppContext| {
        e.app(cx, |app, _w, _cx| {
            app.receita_no_cliente().map(|(id, aj)| (id, aj.exposure))
        })
    };

    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("d", cx);
        cx.emit(Pedido::TelaDoCliente);
    });
    // 🔑 **A foto do site chega pela rede.** A segunda tela mostra a **cópia de
    // trabalho**, e quando ela não está no cache o app a pede ao site: o
    // cliente vê a foto quando ela chega, e não um quadro vazio no lugar.
    e.esperar(cx);
    assert_eq!(
        no_cliente(cx).map(|c| c.0),
        Some("site:d".into()),
        "abre na foto em foco, e não na primeira da grade"
    );

    // Trocar o recorte muda o que está em foco: o cliente acompanha.
    recortar(&e, cx, Filtro::Situacao(Estado::LevadaNoBalcao));
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("b", cx));
    e.esperar(cx);
    assert_eq!(no_cliente(cx).map(|c| c.0), Some("site:b".into()));

    // Na revelação, o gesto ao vivo chega sem passar pela galeria.
    e.revelar_a_do_site(cx, "b");
    e.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.8, cx));
    assert_eq!(no_cliente(cx), Some(("site:b".into(), 0.8)));

    // Voltar à grade mantém a revelada — o cliente não pode ver a foto de antes.
    e.esperar(cx);
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Sessao));
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("b", cx));
    assert_eq!(no_cliente(cx), Some(("site:b".into(), 0.8)));
}
