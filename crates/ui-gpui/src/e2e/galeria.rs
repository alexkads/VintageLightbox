//! 🖼️ Dentro da sessão: a galeria do ensaio, como a rota `[id]` do site.

use biblioteca_core::acervo::{Estado, Filtro};
use domain::services::pos_venda::{EstadoNoBalcao, MudancaDaGaleria};
use gpui::{TestAppContext, VisualTestContext};

use super::{abrir_o_app, abrir_o_ensaio, local, Cenario, Estudio, GALERIA};
use crate::app::Tela;
use crate::impressao::porta::Destino;

/// Clica no botão marcado com `debug_selector`, onde o dedo clicaria.
fn clicar(e: &Estudio, cx: &mut TestAppContext, alvo: &'static str) {
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.run_until_parked();
    let onde = visual
        .debug_bounds(alvo)
        .unwrap_or_else(|| panic!("o botão {alvo} não está desenhado na tela"));
    visual.simulate_click(onde.center(), gpui::Modifiers::none());
    visual.run_until_parked();
}

/// 🚨 **A receita padrão da sessão vale para quem chega depois.**
///
/// A predefinição e a proporção escolhidas na etapa 2 do assistente ficam **na
/// galeria**. Quem importa mais fotos dentro da sessão espera o mesmo visual — e
/// era o que não acontecia: o serviço da receita só atendia o assistente, e a
/// leva seguinte entrava crua (achado do dono, 17/set/2026).
#[gpui::test]
fn a_foto_importada_na_sessao_recebe_a_receita_padrao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| {
                for galeria in site.galerias.lock().unwrap().iter_mut() {
                    if galeria.id == GALERIA {
                        galeria.preset_padrao_id = Some("sistema:sepia".into());
                        galeria.proporcao_padrao = Some("1:1".into());
                    }
                }
            }),
            ..Default::default()
        },
    );

    // As locais do ensaio já entram na conta da receita: elas são as que ainda
    // não subiram, e é nelas que a receita da galeria manda.
    e.app(cx, |app, _w, _cx| {
        assert!(
            app.receita_padrao_pedida() > 0,
            "a receita da galeria tinha de ser pedida para as fotos locais"
        );
    });
}

/// 🧾 **A faixa escolhida na barra sobe com a foto que entra depois dela.**
///
/// É a primeira das duas escolhas antes dos arquivos, no site: a sessão mista
/// sobe a mãe sozinha numa faixa e a família em outra. O campo existia na tela
/// (`escolher_faixa`) e **não chegava ao site** — nem o catálogo de faixas era
/// carregado, então o seletor nem aparecia.
///
/// 🔄 **A ordem passou a importar em 2026-09-20** (C20). Antes a foto subia no
/// gesto da nota, sempre depois da escolha da faixa; agora ela sobe sozinha,
/// assim que entra no ensaio. A faixa da barra vale para o que **entrar depois**
/// dela — como no site, onde ela é escolhida antes de arrastar os arquivos. Para
/// a que já subiu, a faixa é uma mudança da foto, no painel.
#[gpui::test]
fn a_faixa_da_barra_sobe_com_a_foto_que_entra_depois(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.esperar(cx);
    assert_eq!(
        e.site.faixas_pedidas(),
        vec![None, None],
        "as que já estavam no ensaio sobem no padrão da galeria"
    );

    e.detalhe(cx, |tela, _w, cx| {
        assert!(!tela.produtos().is_empty(), "o catálogo chega com a sessão");
        tela.escolher_faixa(Some("p1".into()), cx);
    });

    // Agora a importação: é ela que entra depois da escolha.
    e.acervo.fotos.lock().unwrap().push(local("DSC_201.jpg"));
    clicar(&e, cx, "detalhe-importar");
    e.esperar(cx);

    assert_eq!(
        e.site.faixas_pedidas(),
        vec![None, None, Some("p1".to_string())],
        "a importada tinha de subir na faixa escolhida"
    );
}

/// 🎬 **Importar, classificar e levar**: o botão "Importar" abre o seletor do
/// sistema, o lote vai para o catálogo **local** com o carimbo do ensaio e
/// **sobe sozinho** (C20), a nota marca a foto (C22), o `X` a rejeita sem
/// apagar nada (C21) e o `B` leva a do site.
#[gpui::test]
fn importar_classificar_e_levar_pelas_teclas(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    // A grade é uma só: as quatro do site e as duas locais.
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.total_visivel(), 6, "site e disco na mesma grade");
        assert_eq!(tela.contagem(), (2, 1, 1), "levadas, à venda, compradas");
    });

    // 📥 Importar, pelo clique no botão.
    e.acervo.fotos.lock().unwrap().push(local("DSC_201.jpg"));
    clicar(&e, cx, "detalhe-importar");
    e.esperar(cx);
    assert_eq!(e.seletor_de_fotos.pedidos(), 1, "a janela do sistema abriu");
    let lotes = e.importador.importados();
    assert_eq!(lotes.len(), 1);
    assert_eq!(
        lotes[0].0,
        vec!["/cartao/DSC_201.jpg", "/cartao/DSC_202.jpg"]
    );
    assert_eq!(lotes[0].1.sessao_id.as_deref(), Some(GALERIA));
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.total_visivel(), 7, "a importada entrou na grade");
    });

    // 📤 **O ensaio inteiro sobe sozinho** (C20): as duas que já estavam no
    // disco ao abrir a sessão, e a que acabou de ser importada — nenhuma
    // classificada. 🔄 Até 2026-09-20 nada disto subia: *"importar não sobe
    // nada — quem autoriza é a nota"*.
    let subidas = e.site.subidas();
    assert_eq!(subidas.len(), 3, "o ensaio inteiro sobe: {subidas:?}");
    assert_eq!(subidas[0].0, GALERIA);
    assert_eq!(subidas[2].1, "id-DSC_201.jpg", "a recém-importada também");
    assert!(
        e.site.notas_pedidas().iter().all(Option::is_none),
        "e sobem sem nota: {:?}",
        e.site.notas_pedidas()
    );

    // ⭐ A nota pela tecla, numa foto que já subiu: é uma mudança da foto.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("id-DSC_101.jpg", cx));
    e.teclar(cx, "4");
    e.esperar(cx);
    assert_eq!(e.site.subidas().len(), 3, "classificar não sobe de novo");

    // O `B` numa foto sem nota fica de fora, com a regra do servidor. 🔄 Até
    // 21/set/2026 a foto que ainda subia dava "espere o envio terminar"; agora
    // o `B` vale nela (dono: *"eu não posso impedir o atendente de fazer as
    // marcações"*), e o que a segura é só a nota, como a do site.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("id-DSC_102.jpg", cx));
    e.teclar(cx, "b");
    e.detalhe(cx, |tela, _w, _cx| {
        assert!(
            tela.erro().is_some_and(|f| f.contains("classifique")),
            "sinalizar a que não subiu avisa: {:?}",
            tela.erro()
        );
    });

    // 🛍️ O `B` na foto à venda: ela vira levada no balcão.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "b");
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(negociadas.len(), 1, "{negociadas:?}");
    assert_eq!(negociadas[0].0, "d");
    assert_eq!(negociadas[0].1.estado, Some(EstadoNoBalcao::LevadaNoBalcao));

    // ⭐ E a nota nela, que já está no site, é uma mudança da foto.
    e.teclar(cx, "5");
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(negociadas.len(), 2);
    assert_eq!(negociadas[1].1.nota, Some(Some(5)));

    // 🚨 **O `0` numa foto do acervo tira a nota, e só isso** (contrato C22,
    // 2026-09-20). Até 18/set/2026 ele recusava ("use Apagar"); de lá até
    // 20/set ele abria a pergunta do resgate — o bruto de volta para cá e a
    // foto apagada da nuvem. Nenhum gesto de classificação apaga arquivo.
    e.teclar(cx, "0");
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(
        negociadas.len(),
        3,
        "o 0 é uma mudança da foto: {negociadas:?}"
    );
    assert_eq!(negociadas[2].0, "d");
    assert_eq!(
        negociadas[2].1.nota,
        Some(None),
        "tirar a nota é `nota: null`"
    );
    assert!(
        e.site.tiradas().is_empty(),
        "e nada sai da nuvem — o resgate destrutivo deixou de existir"
    );

    // ❌ **O `X` na foto da nuvem vai ao resgate, e não a um `PATCH`**
    // (2026-09-21): a `d` não tem cópia catalogada aqui, então o bruto é pedido
    // antes — e, sem ele chegar ao catálogo, nada sai da nuvem.
    e.teclar(cx, "x");
    e.esperar(cx);
    assert_eq!(e.site.negociadas().len(), 3, "nenhum PATCH de rejeição");
    assert_eq!(e.site.originais(), ["d"], "o bruto foi pedido primeiro");
    assert!(
        e.site.tiradas().is_empty(),
        "e nada saiu da nuvem sem a cópia"
    );

    // A comprada não muda por lote.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("c", cx));
    e.teclar(cx, "b");
    e.esperar(cx);
    assert_eq!(e.site.negociadas().len(), 3, "a comprada fica de fora");
}

/// O que o catálogo desta máquina sabe da foto: (nota, sinalizador).
fn no_catalogo(e: &Estudio, cx: &mut TestAppContext, id: &str) -> (i32, Option<i32>) {
    e.app(cx, |app, _w, cx| {
        let foto = app
            .biblioteca
            .read(cx)
            .todas_as_fotos()
            .into_iter()
            .find(|f| f.id == id)
            .unwrap_or_else(|| panic!("{id} não está no catálogo"));
        (foto.rating, foto.flag)
    })
}

/// 🚨 **Classificar e sinalizar a foto que ainda não subiu** — o gesto que o
/// operador faz com o cliente na frente, enquanto o ensaio sobe em segundo
/// plano (C20–C22).
///
/// Regressão de 21/set/2026: a passada de subida passou a pular a foto sem
/// nota, e o gesto mais comum da sessão — dar nota e rejeitar durante a
/// importação — deixava de ter efeito. Aqui as teclas de verdade passam pela
/// grade da sessão até o catálogo, e o cenário afirma o que **ficou gravado**.
#[gpui::test]
fn classificar_e_rejeitar_a_foto_que_ainda_nao_subiu(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    let id = "id-DSC_101.jpg";

    // Todo o ensaio subiu sem nota, e a foto continua sendo do disco.
    assert_eq!(e.site.subidas().len(), 2, "as duas locais subiram");
    assert_eq!(no_catalogo(&e, cx, id), (0, None), "sem nota e sem marca");

    // ⭐ `1`–`5` dão a nota no catálogo, sem tocar no site.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto(id, cx));
    for nota in 1..=5 {
        e.teclar(cx, &nota.to_string());
        e.esperar(cx);
        assert_eq!(no_catalogo(&e, cx, id).0, nota, "a tecla {nota} dá a nota");
        // 🚨 E a **célula da grade mostra a estrela**: gravar sem desenhar fazia
        // o gesto parecer não ter efeito nenhum.
        let na_grade = e.detalhe(cx, |tela, _w, _cx| tela.como_esta(id));
        assert_eq!(
            na_grade.map(|(_, n, _)| n),
            Some(Some(nota as u8)),
            "a grade da sessão mostra a nota {nota}"
        );
    }
    assert!(e.site.negociadas().is_empty(), "foto local não negocia");
    assert_eq!(e.site.subidas().len(), 2, "classificar não sobe de novo");

    // `0` tira a nota — e só isso.
    e.teclar(cx, "0");
    e.esperar(cx);
    assert_eq!(no_catalogo(&e, cx, id), (0, None));
    assert_eq!(
        e.detalhe(cx, |tela, _w, _cx| tela.como_esta(id).map(|(_, n, _)| n)),
        Some(None),
        "e a grade some com a estrela"
    );
    assert!(e.site.tiradas().is_empty(), "nada sai da nuvem");

    // ❌ `X` rejeita (C21) e o segundo `X` desfaz, sem apagar arquivo nenhum.
    e.teclar(cx, "x");
    e.esperar(cx);
    assert_eq!(no_catalogo(&e, cx, id).1, Some(-1), "o X rejeita");
    let rejeitadas = |e: &Estudio, cx: &mut TestAppContext| {
        e.detalhe(cx, |tela, _w, _cx| {
            tela.contagens()
                .de(biblioteca_core::acervo::Filtro::Rejeitadas)
        })
    };
    assert_eq!(
        rejeitadas(&e, cx),
        1,
        "e a grade a põe no recorte dela (C21)"
    );
    e.teclar(cx, "x");
    e.esperar(cx);
    assert_eq!(no_catalogo(&e, cx, id).1, Some(0), "o segundo X desfaz");
    assert_eq!(rejeitadas(&e, cx), 0, "e ela volta");
    assert!(e.site.tiradas().is_empty(), "rejeitar não apaga nada");

    // A outra foto não foi tocada por nenhum destes gestos.
    assert_eq!(no_catalogo(&e, cx, "id-DSC_102.jpg"), (0, None));
}

/// 🔑 **A seleção em lote classifica e rejeita todas de uma vez**, na foto
/// local — uma tecla, um desfecho (`⌘A` e depois a tecla).
#[gpui::test]
fn classificar_e_rejeitar_o_lote_de_fotos_locais(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.detalhe(cx, |tela, _w, cx| {
        tela.marcar_ids(
            &["id-DSC_101.jpg".to_string(), "id-DSC_102.jpg".to_string()],
            cx,
        )
    });

    e.teclar(cx, "3");
    e.esperar(cx);
    assert_eq!(no_catalogo(&e, cx, "id-DSC_101.jpg").0, 3);
    assert_eq!(no_catalogo(&e, cx, "id-DSC_102.jpg").0, 3);

    e.teclar(cx, "x");
    e.esperar(cx);
    assert_eq!(no_catalogo(&e, cx, "id-DSC_101.jpg").1, Some(-1));
    assert_eq!(no_catalogo(&e, cx, "id-DSC_102.jpg").1, Some(-1));

    e.teclar(cx, "x");
    e.esperar(cx);
    assert_eq!(no_catalogo(&e, cx, "id-DSC_101.jpg").1, Some(0));
    assert_eq!(no_catalogo(&e, cx, "id-DSC_102.jpg").1, Some(0));
}

/// 📤 **A subida leva a nota que já existe e segura a rejeitada** (C20/C21).
///
/// A classificada sobe com a nota dela; a sem nota sobe sem nota — o `0` do
/// catálogo nunca atravessa como classificação —, e a rejeitada não sobe.
#[gpui::test]
fn a_subida_leva_a_nota_e_segura_a_rejeitada(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    {
        let mut fotos = e.acervo.fotos.lock().unwrap();
        fotos[0].rating = 4;
        fotos[1].flag = Some(-1);
        fotos.push(local("DSC_103.jpg"));
    }
    // O mesmo recado que a importação dá quando o catálogo muda.
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::CatalogoMudou)
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        app.sessoes
            .update(cx, |tela, cx| tela.abrir(GALERIA.into(), cx));
    });
    e.esperar(cx);

    let subidas = e.site.subidas();
    let ids: Vec<&str> = subidas.iter().map(|s| s.1.as_str()).collect();
    assert!(
        ids.contains(&"id-DSC_101.jpg"),
        "a classificada sobe: {ids:?}"
    );
    assert!(
        ids.contains(&"id-DSC_103.jpg"),
        "a sem nota também: {ids:?}"
    );
    assert!(
        !ids.contains(&"id-DSC_102.jpg"),
        "a rejeitada não sobe: {ids:?}"
    );
    let notas = e.site.notas_pedidas();
    assert!(notas.contains(&Some(4)), "a nota 4 foi junto: {notas:?}");
    assert!(
        notas.iter().all(|n| n.is_none_or(|n| (1..=5).contains(&n))),
        "o 0 nunca atravessa como nota: {notas:?}"
    );

    // Tirar a rejeição não some com a foto: ela só volta a ser candidata.
    assert_eq!(no_catalogo(&e, cx, "id-DSC_102.jpg").1, Some(-1));
}

/// 🚨 **A foto que termina de subir continua na grade** — e a tecla continua
/// tendo onde cair.
///
/// Regressão achada rodando o app contra a pilha local (21/set/2026): ao
/// terminar a subida, a releitura do catálogo tirava a foto das locais (ela
/// ganhou id remoto), mas ninguém relia a galeria do site. A grade ia a
/// "Todas 0", e o operador apertava `3` e `X` sobre nada.
#[gpui::test]
fn a_foto_que_subiu_continua_na_grade_e_aceita_a_nota(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    let acervo = e.acervo.clone();
    *e.site.ao_subir.lock().unwrap() = Some(Box::new(move |site, foto_id, ordem| {
        let no_site = format!("site-{foto_id}");
        if let Some(foto) = acervo
            .fotos
            .lock()
            .unwrap()
            .iter_mut()
            .find(|f| f.id == foto_id)
        {
            foto.pos_venda_foto_id = Some(no_site.clone());
        }
        site.fotos_da_sessao.lock().unwrap().push(super::do_site(
            &no_site,
            ordem as i32 + 10,
            domain::services::pos_venda::EstadoDaFotoNoSite::Disponivel,
            None,
        ));
    }));
    e.entrar_na_conta(cx);
    e.app(cx, |app, _w, cx| {
        app.sessoes
            .update(cx, |tela, cx| tela.abrir(GALERIA.into(), cx));
    });
    e.esperar(cx);

    assert_eq!(e.site.subidas().len(), 2, "as duas locais subiram");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.total_visivel(),
            6,
            "as 4 do site e as 2 que acabaram de subir — nenhuma some"
        );
        assert!(tela.como_esta("site-id-DSC_101.jpg").is_some());
    });

    // A tecla cai na foto que acabou de subir, agora do site.
    e.detalhe(cx, |tela, _w, cx| {
        tela.focar_foto("site-id-DSC_101.jpg", cx)
    });
    e.teclar(cx, "3");
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(negociadas.len(), 1, "{negociadas:?}");
    assert_eq!(negociadas[0].0, "site-id-DSC_101.jpg");
    assert_eq!(negociadas[0].1.nota, Some(Some(3)));
}

/// ❌ **Rejeitar a foto da nuvem que tem cópia aqui: ela sai de lá e fica
/// aqui, marcada** (dono, 2026-09-21) — o caminho de quase sempre, porque o
/// ensaio sobe desta máquina e a cópia local continua (C20.1).
///
/// E tirar a rejeição a sobe de novo, sozinha.
#[gpui::test]
fn rejeitar_a_da_nuvem_com_copia_aqui_e_desfazer(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    // A cópia daqui da foto `d` do site.
    {
        let mut copia = local("copia-d.jpg");
        copia.pos_venda_foto_id = Some("d".into());
        e.acervo.fotos.lock().unwrap().push(copia);
    }
    let acervo = e.acervo.clone();
    *e.site.ao_rejeitar.lock().unwrap() = Some(Box::new(move |site, id_local| {
        // O que o use case de verdade faz: a marca e o id remoto mudam juntos.
        if let Some(f) = acervo
            .fotos
            .lock()
            .unwrap()
            .iter_mut()
            .find(|f| f.id == id_local)
        {
            f.flag = Some(-1);
            f.pos_venda_foto_id = None;
        }
        site.fotos_da_sessao.lock().unwrap().retain(|f| f.id != "d");
    }));
    e.entrar_na_conta(cx);
    e.app(cx, |app, _w, cx| {
        app.sessoes
            .update(cx, |tela, cx| tela.abrir(GALERIA.into(), cx));
    });
    e.esperar(cx);
    let subidas_antes = e.site.subidas().len();

    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "x");
    e.esperar(cx);
    e.esperar(cx);

    assert_eq!(
        *e.site.rejeitadas_na_nuvem.lock().unwrap(),
        ["id-copia-d.jpg"],
        "um pedido só, pelo id daqui"
    );
    assert!(
        e.site.originais().is_empty(),
        "com cópia aqui, nada é baixado"
    );
    assert_eq!(no_catalogo(&e, cx, "id-copia-d.jpg").1, Some(-1));
    e.app(cx, |app, _w, _cx| {
        let desfecho = app.ultimo_resgate().expect("o resgate terminou");
        assert_eq!(desfecho.voltaram, ["d.jpg"]);
        assert!(desfecho.ficaram.is_empty());
    });
    e.detalhe(cx, |tela, _w, _cx| {
        assert!(tela.como_esta("d").is_none(), "saiu da nuvem");
        assert!(
            tela.como_esta("id-copia-d.jpg").is_some(),
            "e está na grade, como local"
        );
        assert_eq!(
            tela.contagens()
                .de(biblioteca_core::acervo::Filtro::Rejeitadas),
            1
        );
    });
    assert_eq!(
        e.site.subidas().len(),
        subidas_antes,
        "a rejeitada não sobe de novo"
    );

    // 🔁 Tirar a rejeição: ela volta a subir sozinha.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("id-copia-d.jpg", cx));
    e.teclar(cx, "x");
    e.esperar(cx);
    e.esperar(cx);
    assert_eq!(no_catalogo(&e, cx, "id-copia-d.jpg").1, Some(0));
    assert!(
        e.site.subidas().iter().any(|s| s.1 == "id-copia-d.jpg"),
        "sem a rejeição, ela sobe de novo: {:?}",
        e.site.subidas()
    );
}

/// ❌ **Rejeitar a foto da nuvem sem cópia aqui: o bruto vem antes**
/// (2026-09-21, o resgate de 18/09 readaptado). A nuvem só perde a foto depois
/// de o arquivo estar gravado e catalogado aqui; sem o bruto, nada sai.
#[gpui::test]
fn rejeitar_a_da_nuvem_sem_copia_traz_o_bruto_antes(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| {
                *site.bruto.lock().unwrap() = None;
            }),
            ..Cenario::default()
        },
    );

    // Sem o bruto no site: nada sai da nuvem.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "x");
    e.esperar(cx);
    e.esperar(cx);
    assert_eq!(e.site.originais(), ["d"], "pediu o bruto");
    assert!(
        e.site.tiradas().is_empty(),
        "🚨 sem cópia, a nuvem não perde nada"
    );
    e.app(cx, |app, _w, _cx| {
        let desfecho = app.ultimo_resgate().expect("o resgate terminou");
        assert_eq!(desfecho.ficaram, ["d.jpg"]);
    });

    // Com o bruto: ele é gravado, catalogado, marcado — e só então sai de lá.
    let mut png = Vec::new();
    super::imagem(8, 8, 90)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    *e.site.bruto.lock().unwrap() = Some(png);
    // O importador de mentira não grava no catálogo: o teste grava por ele, no
    // caminho em que o resgate grava o arquivo.
    let caminho = crate::app::resgate::pasta_das_resgatadas().join("d.jpg");
    {
        let mut volta = local("volta-d.jpg");
        volta.path = caminho.to_string_lossy().to_string();
        e.acervo.fotos.lock().unwrap().push(volta);
    }
    e.teclar(cx, "x");
    e.esperar(cx);
    e.esperar(cx);
    assert!(caminho.exists(), "o bruto foi gravado aqui");
    assert_eq!(e.site.tiradas(), ["d"], "e só então a nuvem perdeu a foto");
    assert_eq!(no_catalogo(&e, cx, "id-volta-d.jpg").1, Some(-1), "marcada");
    let _ = std::fs::remove_dir_all(crate::app::resgate::pasta_das_resgatadas());
}

/// 🎬 **Recortes, zoom e marcação**: os chips recortam, a seleção limpa ao
/// trocar de recorte, `⌘A`/`⌘D` marcam e desmarcam, as setas andam, e o zoom
/// da grade anda dentro dos limites.
#[gpui::test]
fn recortes_zoom_e_marcacao_da_grade(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.teclar(cx, "cmd-a");
    e.detalhe(cx, |tela, _w, cx| {
        assert_eq!(tela.quantas_marcadas(), 6, "⌘A marca a grade inteira");
        tela.filtrar(Filtro::Situacao(Estado::LevadaNoBalcao), cx);
        assert_eq!(tela.total_visivel(), 2, "as duas levadas");
        assert_eq!(
            tela.quantas_marcadas(),
            0,
            "trocar o recorte limpa a seleção"
        );
    });

    e.teclar(cx, "ctrl-a");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.marcadas(), vec!["a".to_string(), "b".to_string()]);
    });
    e.teclar(cx, "cmd-d");
    e.detalhe(cx, |tela, _w, cx| {
        assert_eq!(tela.quantas_marcadas(), 0, "⌘D desmarca");
        tela.filtrar(Filtro::SemNota, cx);
        assert_eq!(tela.total_visivel(), 2, "as locais não têm nota");
        tela.filtrar(Filtro::Classificadas, cx);
        assert_eq!(tela.total_visivel(), 4);
        tela.filtrar(Filtro::Situacao(Estado::Comprada), cx);
        assert_eq!(tela.total_visivel(), 1);
        tela.filtrar(Filtro::Todas, cx);
    });

    // As setas andam na grade da sessão, sem dar a volta.
    e.teclar(cx, "right");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(0))
    });
    e.teclar(cx, "right right");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(2))
    });
    e.teclar(cx, "left left left left");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(0))
    });
    // Numa janela estreita a grade tem mais de uma linha, e ↓ desce uma
    // linha inteira — o passo é o número de colunas que cabem.
    cx.simulate_window_resize(e.raiz.into(), gpui::size(gpui::px(760.), gpui::px(700.)));
    cx.run_until_parked();
    let colunas = e.detalhe(cx, |tela, window, _cx| tela.colunas_visiveis(window));
    assert!(
        colunas > 0 && colunas < 6,
        "a janela estreita quebra a grade: {colunas}"
    );
    e.teclar(cx, "down");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.posicao_em_foco(),
            Some(colunas),
            "↓ desce uma linha inteira"
        );
    });
    e.teclar(cx, "up");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(tela.posicao_em_foco(), Some(0))
    });

    // O zoom da grade: aumenta, diminui e para nos limites.
    e.detalhe(cx, |tela, window, cx| {
        let antes = tela.zoom();
        tela.ajustar_zoom(40., window, cx);
        assert!(tela.zoom() > antes);
        for _ in 0..50 {
            tela.ajustar_zoom(40., window, cx);
        }
        let teto = tela.zoom();
        tela.ajustar_zoom(40., window, cx);
        assert_eq!(tela.zoom(), teto, "o zoom tem teto");
        for _ in 0..50 {
            tela.ajustar_zoom(-40., window, cx);
        }
        assert!(tela.zoom() < antes, "e desce até o piso");
    });
}

/// 🎬 **Negociar e imprimir as marcadas**: os botões da barra levam a seleção
/// da grade ao balcão e à folha — com o id do site atravessando para a
/// Biblioteca, que é quem o balcão lê.
#[gpui::test]
fn negociar_e_imprimir_as_marcadas(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, _w, cx| {
        tela.marcar_ids(&["a".into(), "d".into()], cx);
        let marcadas = tela.marcadas();
        cx.emit(crate::sessoes::detalhe::Pedido::Negociar(marcadas));
    });
    e.app(cx, |app, _w, cx| {
        assert!(app.no_balcao(), "o balcão abriu");
        let negociaveis: Vec<String> = app
            .balcao
            .read(cx)
            .negociaveis()
            .iter()
            .filter_map(|f| f.pos_venda_foto_id.clone())
            .collect();
        assert_eq!(negociaveis, vec!["a".to_string(), "d".to_string()]);
        app.balcao.update(cx, |tela, cx| tela.registrar(cx));
    });
    e.esperar(cx);
    let negociadas = e.site.negociadas();
    assert_eq!(
        negociadas
            .iter()
            .map(|(id, _)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "d"],
        "o acerto foi para as duas marcadas"
    );
    assert!(negociadas
        .iter()
        .all(|(_, m)| m.observacao_da_negociacao.is_some()));
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.balcao.read(cx).gravadas(), 2);
        app.fechar_balcao(cx);
        assert!(!app.no_balcao());
    });

    // 🖨️ Imprimir as mesmas.
    e.detalhe(cx, |tela, _w, cx| {
        let marcadas = tela.marcadas();
        cx.emit(crate::sessoes::detalhe::Pedido::Imprimir(marcadas));
    });
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Impressao);
        let folha = app.impressao_para_teste();
        assert_eq!(folha.read(cx).escolhidas(), 2);
        folha.update(cx, |tela, cx| tela.imprimir(cx));
    });
    e.esperar(cx);
    let pedidos = e.folha.pedidos();
    assert_eq!(pedidos.len(), 1);
    assert_eq!(
        pedidos[0].0,
        vec!["site:a".to_string(), "site:d".to_string()]
    );
    assert_eq!(pedidos[0].2, Destino::Impressora);

    // Esc sai da folha de volta para a galeria, com a sessão aberta.
    e.teclar(cx, "escape");
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao);
        assert!(app.pode_trabalhar());
    });
}

/// 🎬 **Exportar**: o botão da barra abre o modal com a grade, a pasta vem do
/// seletor, e o lote vai para o exportador — nada de pasta real.
#[gpui::test]
fn exportar_pelo_botao_da_barra(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    let pasta = tempfile::TempDir::new().expect("pasta de destino");

    e.detalhe(cx, |tela, _w, cx| {
        tela.marcar_ids(&["id-DSC_101.jpg".into()], cx)
    });
    clicar(&e, cx, "detalhe-exportar");
    e.app(cx, |app, _w, cx| {
        assert!(app.exportando(), "o modal abriu");
        let modal = app.exportacao_para_teste();
        assert!(modal.read(cx).quantas() >= 1);
        let destino = pasta.path().to_path_buf();
        modal.update(cx, |tela, cx| {
            tela.escolher_pasta_para_teste(destino, cx);
            tela.exportar(cx);
        });
    });
    e.esperar(cx);
    let pedidos = e.exportador.pedidos();
    assert_eq!(pedidos.len(), 1, "um lote");
    assert!(pedidos[0]
        .iter()
        .all(|s| s.destino.starts_with(pasta.path())));
    e.app(cx, |app, window, cx| {
        app.fechar_exportacao(window, cx);
        assert!(!app.exportando());
    });
}

/// 🎬 **O fim da sessão**: editar os dados do cliente, copiar o link (que vai
/// para a área de transferência) e avisar o cliente.
#[gpui::test]
fn dados_do_cliente_link_e_aviso(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, window, cx| {
        assert!(tela.tem_email());
        tela.editar_dados_do_cliente(cx);
        tela.digitar_dados_do_cliente(Some("Ensaio da Ana e do Rui"), None, None, window, cx);
        tela.gravar_dados_do_cliente(cx);
    });
    e.esperar(cx);
    assert_eq!(
        e.site.atualizacoes(),
        vec![(
            GALERIA.to_string(),
            MudancaDaGaleria {
                titulo: Some("Ensaio da Ana e do Rui".into()),
                ..Default::default()
            }
        )],
        "o PATCH leva só o que mudou"
    );
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.aberta().map(|a| a.galeria.titulo.clone()).as_deref(),
            Some("Ensaio da Ana e do Rui")
        );
    });

    // 🔗 Copiar link.
    e.detalhe(cx, |tela, _w, cx| tela.pedir_o_link(cx));
    e.esperar(cx);
    assert_eq!(e.site.links(), vec![GALERIA.to_string()]);
    let url = e.detalhe(cx, |tela, _w, _cx| tela.link().map(|l| l.url.clone()));
    let url = url.expect("o link chegou");
    assert_eq!(
        cx.read_from_clipboard().and_then(|c| c.text()),
        Some(url),
        "o link foi para a área de transferência"
    );
    // 🚨 **E ele não expira** (dono, 2026-09-20): o site responde
    // `validade_em_segundos: null`, e eram 7 dias até então. O que este degrau
    // fixa é a tela **atravessar** o `null` — a forma de falhar era a resposta
    // inteira ser recusada, com o link já assinado do outro lado.
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.link().and_then(|l| l.validade_em_segundos),
            None,
            "o link da galeria não expira"
        );
    });

    // 📧 Avisar o cliente.
    e.detalhe(cx, |tela, _w, cx| tela.avisar(cx));
    e.esperar(cx);
    assert_eq!(e.site.avisadas(), vec![GALERIA.to_string()]);

    // ← Voltar: a sessão fecha e a lista volta.
    e.detalhe(cx, |_tela, _w, cx| {
        cx.emit(crate::sessoes::detalhe::Pedido::Voltar)
    });
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessoes);
        assert!(!app.pode_trabalhar());
    });
}

/// 🎬 **A sessão sem e-mail**: o link pede o contato, grava o e-mail e segue o
/// gesto — sem um segundo clique.
#[gpui::test]
fn o_link_de_uma_sessao_sem_email_pede_o_contato(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.app(cx, |app, _w, cx| app.entrar_na_sessao("g2".into(), cx));
    e.esperar(cx);

    e.detalhe(cx, |tela, window, cx| {
        assert_eq!(tela.galeria_id(), Some("g2"));
        assert!(!tela.tem_email());
        tela.pedir_o_link(cx);
        assert!(tela.motivo_do_formulario().is_some(), "o contato é pedido");
        tela.digitar_dados_do_cliente(None, Some("joao@exemplo.com"), None, window, cx);
        tela.gravar_dados_do_cliente(cx);
    });
    e.esperar(cx);
    assert_eq!(e.site.links(), vec!["g2".to_string()], "o gesto seguiu");
    e.detalhe(cx, |tela, _w, _cx| {
        assert!(tela.link().is_some());
        assert!(tela.motivo_do_formulario().is_none());
    });
}

/// 🎬 **`Esc` na galeria não mexe na grade.** O recorte e a seleção que o
/// operador escolheu ficam — o `Esc` é da Revelação e das ferramentas dela.
#[gpui::test]
fn esc_na_galeria_nao_troca_o_recorte_nem_a_selecao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    // A tira da Revelação tem outro recorte e outra seleção.
    e.revelar_a_do_site(cx, "a");
    e.teclar(cx, "escape");
    e.detalhe(cx, |tela, _w, cx| {
        tela.filtrar(Filtro::Situacao(Estado::Comprada), cx);
        tela.marcar_ids(&["c".into()], cx);
    });

    e.teclar(cx, "escape");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.filtro(),
            Filtro::Situacao(Estado::Comprada),
            "o recorte ficou"
        );
        assert_eq!(tela.marcadas(), vec!["c".to_string()], "a seleção ficou");
    });
    e.app(cx, |app, _w, _cx| {
        assert_eq!(app.tela(), Tela::Sessao);
        assert!(app.pode_trabalhar());
    });
}
