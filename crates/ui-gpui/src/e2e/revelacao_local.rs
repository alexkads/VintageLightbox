//! 🖌️ A Revelação local pelo teclado.

use gpui_kit::{Modifiers, TestAppContext, VisualTestContext};

use super::{abrir_o_ensaio, Cenario};

/// 🎬 **`[` e `]` com o pincel na mão mudam o tamanho, não giram a foto**;
/// sem ferramenta, voltam a girar.
#[gpui_kit::test]
fn colchetes_mudam_o_tamanho_do_pincel(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");

    e.teclar(cx, "k");
    let antes = e.revelacao(cx, |tela, _w, _cx| {
        assert!(tela.com_ferramenta_local(), "K pega o pincel");
        tela.raio_local()
    });
    e.teclar(cx, "]");
    let maior = e.revelacao(cx, |tela, _w, _cx| {
        assert_eq!(tela.corte_na_ferramenta().rotation_90(), 0, "] não gira");
        tela.raio_local()
    });
    assert!(maior > antes, "] aumenta: {antes} → {maior}");
    e.teclar(cx, "[ [");
    let menor = e.revelacao(cx, |tela, _w, _cx| tela.raio_local());
    assert!(menor < antes, "[ diminui: {antes} → {menor}");

    // 🚨 O cursor esquecido na busca de predefinições: ali `]` é texto.
    e.revelacao(cx, |tela, window, cx| tela.focar_a_busca(window, cx));
    e.teclar(cx, "]");
    assert_eq!(
        e.revelacao(cx, |tela, _w, _cx| tela.raio_local()),
        menor,
        "no campo, ] é texto"
    );
    // Clicar na foto devolve o teclado aos atalhos.
    let meio = e.revelacao(cx, |tela, _w, _cx| tela.meio_do_palco());
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_click(meio, Modifiers::default());
    visual.run_until_parked();
    e.teclar(cx, "]");
    let depois = e.revelacao(cx, |tela, _w, _cx| tela.raio_local());
    assert!(
        depois > menor,
        "clicar na foto tira o foco do campo: {menor} → {depois}"
    );
}

/// 🎬 **Um clique de carimbo ou band-aid vira retoque, e o carimbo acha a
/// origem sozinho** (achado no app real, 2026-09-26: sem ⌥-clique ele não
/// fazia nada).
///
/// ⚠️ O outro achado da mesma rodada — apertar e soltar no mesmo quadro
/// deixava o gesto preso, porque o soltar só era ouvido pela janela a partir
/// do desenho seguinte — **não aparece aqui**: o harness redesenha entre os
/// dois eventos. Ele foi provado com `VLB_ROTEIRO` + `mouse_real clicar`.
#[gpui_kit::test]
fn clique_rapido_de_carimbo_e_band_aid_vira_retoque(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    let meio = e.revelacao(cx, |tela, _w, _cx| tela.meio_do_palco());

    // Pontos diferentes: clicar sobre um retoque o seleciona, não cria outro.
    for (tecla, dx, total) in [("s", -200., 1), ("j", 200., 2)] {
        e.teclar(cx, tecla);
        let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
        let ponto = gpui_kit::point(meio.x + gpui_kit::px(dx), meio.y);
        visual.simulate_click(ponto, Modifiers::default());
        visual.run_until_parked();
        let (retoques, preso) = e.revelacao(cx, |tela, _w, _cx| {
            (tela.retoques_locais(), tela.gesto_local_em_curso())
        });
        assert_eq!(retoques, total, "{tecla}: o clique vira retoque");
        assert!(!preso, "{tecla}: e o gesto não fica preso");
    }
}

/// 🔎 **O Tamanho é na tela** (dono, 27/set/2026): aproximar afina a
/// ferramenta sozinho, como no Lightroom — o círculo fica do mesmo tamanho na
/// tela e cobre menos da foto. E o carimbo já dado **não muda** com o zoom: ele
/// guarda o raio em fração da foto.
#[gpui_kit::test]
fn o_zoom_afina_a_ferramenta_e_o_retoque_feito_fica(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.teclar(cx, "s");
    let encaixada = e.revelacao(cx, |tela, _w, _cx| tela.raio_local());

    // Um carimbo com a foto inteira à vista.
    let meio = e.revelacao(cx, |tela, _w, _cx| tela.meio_do_palco());
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_click(meio, Modifiers::default());
    visual.run_until_parked();
    let raio_do_carimbo = |e: &super::Estudio, cx: &mut TestAppContext| {
        e.revelacao(cx, |tela, _w, _cx| {
            tela.locais().retoques.first().map(|r| r.traco().raio)
        })
    };
    let feito = raio_do_carimbo(&e, cx).expect("o clique vira carimbo");
    assert!(
        (feito - encaixada).abs() < 1e-6,
        "o carimbo leva o raio da ferramenta"
    );

    let escala_encaixada = e
        .revelacao(cx, |tela, _w, _cx| tela.escala_local())
        .expect("a foto tem medida");
    for _ in 0..4 {
        e.revelacao(cx, |tela, _w, cx| tela.passo_de_zoom(1, cx));
        let visual = VisualTestContext::from_window(e.raiz.into(), cx);
        visual.run_until_parked();
    }
    let (ampliada, escala) = e.revelacao(cx, |tela, _w, _cx| {
        (
            tela.raio_local(),
            tela.escala_local().expect("a foto tem medida"),
        )
    });
    assert!(
        escala > escala_encaixada * 1.5,
        "os passos aproximam: {escala_encaixada} → {escala}"
    );
    assert!(
        ampliada < encaixada,
        "com zoom, o mesmo círculo cobre menos da foto: {encaixada} → {ampliada}"
    );
    assert!(
        (ampliada * escala - encaixada * escala_encaixada).abs() < 0.01,
        "o tamanho na tela é o mesmo: {} → {} pontos",
        encaixada * escala_encaixada,
        ampliada * escala
    );
    assert_eq!(
        raio_do_carimbo(&e, cx),
        Some(feito),
        "o carimbo já dado não muda com o zoom"
    );
}

/// 🚪 **O botão de sair larga tudo de uma vez** (dono, 27/set/2026): a
/// ferramenta, a máscara escolhida e o retoque selecionado — o que o `Esc`
/// faz em vários toques. Fora da Revelação local ele fica apagado.
#[gpui_kit::test]
fn o_botao_de_sair_larga_a_revelacao_local(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");

    // Um carimbo feito e selecionado, com o carimbo ainda na mão.
    e.teclar(cx, "s");
    let meio = e.revelacao(cx, |tela, _w, _cx| tela.meio_do_palco());
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_click(meio, Modifiers::default());
    visual.run_until_parked();
    visual.simulate_click(meio, Modifiers::default());
    visual.run_until_parked();
    assert!(e.revelacao(cx, |tela, _w, _cx| tela.com_ferramenta_local()));

    // A coluna rola: o painel da Revelação local fica abaixo do Básico.
    let mut visual = super::chatbot::quadro_novo(&e, cx);
    let coluna = visual
        .debug_bounds("local-marcacoes")
        .expect("o botão das marcações está na barra");
    visual.simulate_event(gpui_kit::ScrollWheelEvent {
        position: gpui_kit::point(coluna.left(), gpui_kit::px(400.)),
        delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
            gpui_kit::px(0.),
            gpui_kit::px(-600.),
        )),
        ..Default::default()
    });
    let mut visual = super::chatbot::quadro_novo(&e, cx);
    let sair = visual
        .debug_bounds("local-sair")
        .expect("o botão de sair está na barra");
    let marcacoes = visual
        .debug_bounds("local-marcacoes")
        .expect("o botão das marcações está na barra");
    assert!(
        sair.left() >= marcacoes.right() && sair.top() < marcacoes.bottom(),
        "o sair fica no fim da barra, na mesma linha: {sair:?} × {marcacoes:?}"
    );
    super::chatbot::clicar(&e, cx, "local-sair");

    let (dentro, com_ferramenta, retoques) = e.revelacao(cx, |tela, _w, _cx| {
        (
            tela.dentro_da_revelacao_local(),
            tela.com_ferramenta_local(),
            tela.retoques_locais(),
        )
    });
    assert!(!com_ferramenta, "a ferramenta sai da mão");
    assert!(!dentro, "nada fica escolhido");
    assert_eq!(retoques, 1, "sair não apaga o que foi feito");
}

// ---------------------------------------------------------------------------
// 🧪 A bateria de usabilidade (dono, 03/out/2026: *"teste com muito critério a
// usabilidade UX de todas as funções de Revelação local"*). Cada função do
// painel, pelo ponteiro e pelo teclado de verdade — e cada defeito achado na
// auditoria tem o seu teste aqui, para não voltar.
// ---------------------------------------------------------------------------

use gpui_kit::{Bounds, MouseButton, Pixels, Point};
use revelacao_core::locais::{Forma, Modo, Retoque};

use super::chatbot::quadro_novo;
use super::Estudio;
use crate::app::Tela;
use crate::revelacao::tela::{Ferramenta, VistaLocal};

fn vista(e: &Estudio, cx: &mut TestAppContext) -> VistaLocal {
    // Um desenho antes: o trilho da Exposição se acerta no desenho.
    quadro_novo(e, cx);
    e.revelacao(cx, |tela, _w, cx| tela.vista_local(cx))
}

fn camadas(e: &Estudio, cx: &mut TestAppContext) -> Vec<revelacao_core::locais::Camada> {
    e.revelacao(cx, |tela, _w, _cx| tela.locais().camadas.clone())
}

fn retoques(e: &Estudio, cx: &mut TestAppContext) -> Vec<Retoque> {
    e.revelacao(cx, |tela, _w, _cx| tela.locais().retoques.clone())
}

fn tela_do_app(e: &Estudio, cx: &mut TestAppContext) -> Tela {
    e.app(cx, |app, _w, _cx| app.tela())
}

/// O ponto `q` da foto (0–1) em pixels da janela.
fn na_foto(e: &Estudio, cx: &mut TestAppContext, q: [f32; 2]) -> Point<Pixels> {
    e.revelacao(cx, |tela, _w, _cx| {
        tela.na_janela_da_foto(q).expect("a foto está no palco")
    })
}

/// Um arrasto de verdade pela foto: aperta no primeiro ponto, passa pelos
/// outros e solta no último.
fn arrastar(e: &Estudio, cx: &mut TestAppContext, caminho: &[[f32; 2]], m: Modifiers) {
    let pontos: Vec<Point<Pixels>> = caminho.iter().map(|q| na_foto(e, cx, *q)).collect();
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    // O primeiro evento de ponteiro da janela se perde no harness.
    visual.simulate_mouse_move(pontos[0], None, m);
    visual.simulate_mouse_down(pontos[0], MouseButton::Left, m);
    visual.run_until_parked();
    for p in &pontos[1..] {
        visual.simulate_mouse_move(*p, Some(MouseButton::Left), m);
        visual.run_until_parked();
    }
    let fim = *pontos.last().expect("um caminho");
    visual.simulate_mouse_up(fim, MouseButton::Left, m);
    visual.run_until_parked();
}

fn clicar_na_foto(e: &Estudio, cx: &mut TestAppContext, q: [f32; 2], m: Modifiers) {
    let p = na_foto(e, cx, q);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_mouse_move(p, None, m);
    visual.simulate_click(p, m);
    visual.run_until_parked();
}

fn duplo_clique_na_foto(e: &Estudio, cx: &mut TestAppContext, q: [f32; 2]) {
    let p = na_foto(e, cx, q);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_mouse_move(p, None, Modifiers::default());
    for vezes in [1, 2] {
        visual.simulate_event(gpui_kit::MouseDownEvent {
            button: MouseButton::Left,
            position: p,
            modifiers: Modifiers::default(),
            click_count: vezes,
            first_mouse: false,
        });
        visual.simulate_event(gpui_kit::MouseUpEvent {
            button: MouseButton::Left,
            position: p,
            modifiers: Modifiers::default(),
            click_count: vezes,
        });
        visual.run_until_parked();
    }
}

/// Rola a coluna até `alvo` ficar no meio da janela, e devolve onde ele está.
/// O painel da Revelação local fica abaixo do Básico, fora da dobra.
fn rolar_ate(e: &Estudio, cx: &mut TestAppContext, alvo: &str) -> Bounds<Pixels> {
    let alvo: &'static str = Box::leak(alvo.to_string().into_boxed_str());
    let mut anterior = None;
    for _ in 0..30 {
        let mut visual = quadro_novo(e, cx);
        let altura = f32::from(visual.update(|w, _| w.viewport_size().height));
        let onde = visual
            .debug_bounds(alvo)
            .unwrap_or_else(|| panic!("{alvo} não está desenhado na tela"));
        let meio = f32::from(onde.center().y);
        let falta = meio - altura * 0.55;
        if falta.abs() < altura * 0.2 || anterior == Some(meio) {
            return onde;
        }
        anterior = Some(meio);
        let barra = visual
            .debug_bounds("local-barra")
            .expect("o painel da Revelação local está aberto");
        let sobre = gpui_kit::point(barra.center().x, gpui_kit::px(altura * 0.55));
        visual.simulate_mouse_move(sobre, None, Modifiers::default());
        visual.simulate_event(gpui_kit::ScrollWheelEvent {
            position: sobre,
            delta: gpui_kit::ScrollDelta::Pixels(gpui_kit::point(
                gpui_kit::px(0.),
                gpui_kit::px(-falta.clamp(-400., 400.)),
            )),
            ..Default::default()
        });
        visual.run_until_parked();
    }
    panic!("a coluna não rolou até {alvo}")
}

/// Um clique de verdade num controle do painel da Revelação local.
fn clicar_no_painel(e: &Estudio, cx: &mut TestAppContext, alvo: &str) {
    let onde = rolar_ate(e, cx, alvo);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_mouse_move(onde.center(), None, Modifiers::default());
    visual.simulate_click(onde.center(), Modifiers::default());
    visual.run_until_parked();
}

/// O ensaio aberto na foto "a", com o caixa flutuante minimizado (`F9`): ele
/// cobre o canto de baixo da coluna, e os cliques na lista de máscaras caíam
/// nele.
fn na_foto_a(cx: &mut TestAppContext) -> Estudio {
    let e = abrir_o_ensaio(cx, Cenario::default());
    e.revelar_a_do_site(cx, "a");
    e.teclar(cx, "f9");
    assert!(
        e.app(cx, |app, _w, cx| app.caixa_flutuante.read(cx).minimizado()),
        "o F9 minimiza o caixa flutuante"
    );
    // O Básico fechado traz a Revelação local para perto do topo da coluna.
    e.revelacao(cx, |tela, _w, cx| {
        tela.seguir_o_roteiro_do_painel("fechar Básico", cx)
    });
    e
}

fn alt() -> Modifiers {
    Modifiers {
        alt: true,
        ..Default::default()
    }
}

/// 📏 **A barra inteira cabe na coluna mais estreita** (print do dono,
/// 03/out/2026: o "Sair" saía cortado pela borda). Com 280 pontos — o mínimo
/// da coluna — as sete ferramentas, o zoom, as marcações e o sair ficam dentro.
#[gpui_kit::test]
fn a_barra_cabe_na_coluna_mais_estreita(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.revelacao(cx, |tela, window, cx| {
        tela.largura_da_coluna(280., window, cx);
        tela.abrir_o_painel_local(cx);
    });
    let barra = rolar_ate(&e, cx, "local-barra");
    let mut visual = quadro_novo(&e, cx);
    for alvo in [
        "local-pincel",
        "local-linear",
        "local-radial",
        "local-laco",
        "local-carimbo",
        "local-bandaid",
        "local-preencher",
        "local-zoom",
        "local-marcacoes",
        "local-sair",
    ] {
        let botao = visual
            .debug_bounds(alvo)
            .unwrap_or_else(|| panic!("{alvo} está na barra"));
        assert!(
            botao.right() <= barra.right() + gpui_kit::px(0.5)
                && botao.left() >= barra.left() - gpui_kit::px(0.5),
            "{alvo} sai da barra: {botao:?} × {barra:?}"
        );
    }
}

/// ⌨️ **Cada atalho pega a sua ferramenta e abre o painel; a mesma tecla de
/// novo a solta.** E o botão da barra faz o mesmo que a tecla.
#[gpui_kit::test]
fn atalhos_e_botoes_pegam_e_soltam_cada_ferramenta(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    for (tecla, botao, ferramenta) in [
        ("k", "local-pincel", Ferramenta::Pincel),
        ("m", "local-linear", Ferramenta::Linear),
        ("shift-m", "local-radial", Ferramenta::Radial),
        ("l", "local-laco", Ferramenta::Laco),
        ("s", "local-carimbo", Ferramenta::Carimbo),
        ("j", "local-bandaid", Ferramenta::BandAid),
        ("shift-j", "local-preencher", Ferramenta::Preencher),
    ] {
        e.teclar(cx, tecla);
        assert_eq!(vista(&e, cx).ferramenta, Some(ferramenta), "{tecla} pega");
        assert!(
            super::chatbot::desenhado(&e, cx, "local-barra"),
            "{tecla} abre o painel"
        );
        e.teclar(cx, tecla);
        assert_eq!(vista(&e, cx).ferramenta, None, "{tecla} de novo solta");

        clicar_no_painel(&e, cx, botao);
        assert_eq!(vista(&e, cx).ferramenta, Some(ferramenta), "{botao} pega");
        clicar_no_painel(&e, cx, botao);
        assert_eq!(vista(&e, cx).ferramenta, None, "{botao} de novo solta");
    }
    assert_eq!(tela_do_app(&e, cx), Tela::Revelacao);
}

/// 🖌️ **O pincel: o 1º traço cria a Máscara 1, o 2º soma nela, o ⌥ subtrai,
/// e cada pincelada é um ⌘Z.**
#[gpui_kit::test]
fn o_pincel_cria_soma_subtrai_e_desfaz_por_pincelada(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(
        &e,
        cx,
        &[[0.3, 0.4], [0.35, 0.4], [0.4, 0.4]],
        Modifiers::default(),
    );
    let c = camadas(&e, cx);
    assert_eq!(c.len(), 1, "o primeiro traço cria a máscara");
    assert_eq!(c[0].nome, "Máscara 1");
    assert!((c[0].ajustes.exposicao_ev - 0.8).abs() < 1e-6);
    assert_eq!(vista(&e, cx).mascara_sel, Some(0), "e a deixa escolhida");

    arrastar(&e, cx, &[[0.3, 0.6], [0.4, 0.6]], Modifiers::default());
    arrastar(&e, cx, &[[0.35, 0.3], [0.35, 0.7]], alt());
    let c = camadas(&e, cx);
    assert_eq!(c.len(), 1, "os traços seguintes ficam na mesma máscara");
    let modos: Vec<Modo> = c[0].componentes.iter().map(|k| k.modo).collect();
    assert_eq!(modos, vec![Modo::Somar, Modo::Somar, Modo::Subtrair]);

    e.teclar(cx, "cmd-z");
    assert_eq!(
        camadas(&e, cx)[0].componentes.len(),
        2,
        "um ⌘Z por pincelada"
    );
    e.teclar(cx, "cmd-shift-z");
    assert_eq!(camadas(&e, cx)[0].componentes.len(), 3, "⌘⇧Z refaz");
}

/// 🎚️ **O trilho da Exposição é o da máscara escolhida** — ao criar, no duplo
/// clique, na lista e depois de ⌘Z/⌘⇧Z. O número ao lado dizia +0,80 e o
/// trilho seguia no valor de antes; o arrasto saltava.
#[gpui_kit::test]
fn o_trilho_da_exposicao_e_o_da_mascara_escolhida(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    let v = vista(&e, cx);
    assert!(
        (v.trilho_da_exposicao - 0.8).abs() < 1e-3,
        "a máscara nasce com +0,80, e o trilho também: {}",
        v.trilho_da_exposicao
    );

    e.revelacao(cx, |tela, _w, cx| tela.soltar_a_exposicao_em(-1.5, cx));
    e.teclar(cx, "cmd-z");
    let (ev, v) = (camadas(&e, cx)[0].ajustes.exposicao_ev, vista(&e, cx));
    assert!((ev - 0.8).abs() < 1e-6, "⌘Z volta a exposição");
    assert!(
        (v.trilho_da_exposicao - ev).abs() < 1e-3,
        "e o trilho vai junto: {} × {ev}",
        v.trilho_da_exposicao
    );
    e.teclar(cx, "cmd-shift-z");
    let v = vista(&e, cx);
    assert!(
        (v.trilho_da_exposicao + 1.5).abs() < 1e-3,
        "⌘⇧Z também: {}",
        v.trilho_da_exposicao
    );

    // Uma segunda máscara, com outra exposição; o duplo clique volta à 1ª.
    clicar_no_painel(&e, cx, "nova-mascara-radial");
    arrastar(&e, cx, &[[0.7, 0.7], [0.8, 0.8]], Modifiers::default());
    assert_eq!(camadas(&e, cx).len(), 2);
    e.revelacao(cx, |tela, _w, cx| tela.soltar_a_exposicao_em(2.0, cx));
    e.teclar(cx, "escape");
    e.teclar(cx, "escape");
    duplo_clique_na_foto(&e, cx, [0.35, 0.4]);
    let v = vista(&e, cx);
    assert_eq!(v.mascara_sel, Some(0), "o duplo clique escolhe a do pincel");
    assert!(
        (v.trilho_da_exposicao + 1.5).abs() < 1e-3,
        "com o trilho dela: {}",
        v.trilho_da_exposicao
    );
    clicar_no_painel(&e, cx, "mascara-1");
    let v = vista(&e, cx);
    assert_eq!(v.mascara_sel, Some(1));
    assert!(
        (v.trilho_da_exposicao - 2.0).abs() < 1e-3,
        "pela lista também"
    );
}

/// ⎋ **O `Esc` larga uma coisa por vez, e a máscara escolhida é uma delas**:
/// o laço pela metade, a ferramenta, a máscara — e só então sai da
/// Revelação. Com a máscara escolhida, o 2º `Esc` fechava a tela inteira.
#[gpui_kit::test]
fn esc_larga_a_mascara_antes_de_sair_da_revelacao(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());

    // O laço poligonal pela metade é o primeiro a sair.
    e.teclar(cx, "l");
    clicar_no_painel(&e, cx, "local-laco-1");
    clicar_na_foto(&e, cx, [0.6, 0.6], Modifiers::default());
    clicar_na_foto(&e, cx, [0.7, 0.6], Modifiers::default());
    assert_eq!(vista(&e, cx).poligono, 2, "dois vértices");
    e.teclar(cx, "escape");
    let v = vista(&e, cx);
    assert_eq!(v.poligono, 0, "o Esc larga o laço");
    assert_eq!(
        v.ferramenta,
        Some(Ferramenta::Laco),
        "e fica com a ferramenta"
    );

    e.teclar(cx, "escape");
    let v = vista(&e, cx);
    assert_eq!(v.ferramenta, None, "o 2º larga a ferramenta");
    assert_eq!(v.mascara_sel, Some(0), "a máscara continua escolhida");
    assert_eq!(tela_do_app(&e, cx), Tela::Revelacao);

    e.teclar(cx, "escape");
    assert_eq!(
        tela_do_app(&e, cx),
        Tela::Revelacao,
        "com a máscara escolhida, o Esc a larga — não fecha a Revelação"
    );
    assert_eq!(vista(&e, cx).mascara_sel, None);
    assert!(!e.revelacao(cx, |tela, _w, _cx| tela.dentro_da_revelacao_local()));

    e.teclar(cx, "escape");
    assert_ne!(tela_do_app(&e, cx), Tela::Revelacao, "agora sim, sai");
    assert_eq!(camadas(&e, cx).len(), 1, "e nada se perdeu");
}

/// 🗑️ **A lixeira do retoque apaga e não escolhe outro.** O clique descia da
/// lixeira para a linha, que escolhia o índice que acabara de ser apagado
/// — outro retoque, ou nenhum.
#[gpui_kit::test]
fn a_lixeira_do_retoque_nao_escolhe_outro(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "s");
    for x in [0.25, 0.5, 0.75] {
        clicar_na_foto(&e, cx, [x, 0.3], Modifiers::default());
    }
    assert_eq!(retoques(&e, cx).len(), 3);
    e.teclar(cx, "escape");
    assert_eq!(vista(&e, cx).selecionado, None, "o Esc larga a seleção");

    clicar_no_painel(&e, cx, "apagar-retoque-0");
    assert_eq!(retoques(&e, cx).len(), 2, "a lixeira apaga");
    assert_eq!(
        vista(&e, cx).selecionado,
        None,
        "e não escolhe o que ficou no lugar"
    );
}

/// 🫥 **O círculo da ferramenta some quando o ponteiro sai da foto.** Ele
/// ficava parado onde o ponteiro saiu, como se a ferramenta estivesse ali.
#[gpui_kit::test]
fn o_circulo_some_quando_o_ponteiro_sai_da_foto(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    let dentro = na_foto(&e, cx, [0.5, 0.5]);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_mouse_move(dentro, None, Modifiers::default());
    visual.simulate_mouse_move(
        gpui_kit::point(dentro.x + gpui_kit::px(1.), dentro.y),
        None,
        Modifiers::default(),
    );
    visual.run_until_parked();
    assert!(vista(&e, cx).com_cursor, "sobre a foto, o círculo aparece");

    let barra = rolar_ate(&e, cx, "local-barra");
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_mouse_move(barra.center(), None, Modifiers::default());
    visual.run_until_parked();
    assert!(
        !vista(&e, cx).com_cursor,
        "no painel, o círculo não fica na foto"
    );
}

/// 👁️ **Escolher a máscara na lista a mostra na foto**: pega a ferramenta do
/// componente, e o contorno e as alças aparecem — como no duplo clique.
#[gpui_kit::test]
fn escolher_a_mascara_na_lista_mostra_as_alcas(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "m");
    arrastar(
        &e,
        cx,
        &[[0.5, 0.2], [0.5, 0.4], [0.5, 0.6]],
        Modifiers::default(),
    );
    assert_eq!(camadas(&e, cx).len(), 1);
    e.teclar(cx, "s");
    assert_eq!(vista(&e, cx).mascara_sel, None, "o carimbo larga a máscara");

    clicar_no_painel(&e, cx, "mascara-0");
    let v = vista(&e, cx);
    assert_eq!(v.mascara_sel, Some(0));
    assert_eq!(
        v.ferramenta,
        Some(Ferramenta::Linear),
        "a ferramenta é a do componente"
    );
    assert!(v.marcas >= 4, "as três retas e as alças: {}", v.marcas);
}

/// ⌫ **Delete apaga a máscara escolhida** (sem retoque escolhido), num passo
/// que o ⌘Z desfaz.
#[gpui_kit::test]
fn delete_apaga_a_mascara_escolhida(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    e.teclar(cx, "delete");
    assert!(camadas(&e, cx).is_empty(), "Delete apaga a máscara");
    assert_eq!(vista(&e, cx).mascara_sel, None);
    assert_eq!(tela_do_app(&e, cx), Tela::Revelacao);
    e.teclar(cx, "cmd-z");
    assert_eq!(camadas(&e, cx).len(), 1, "⌘Z a traz de volta");

    // Com um retoque escolhido, o Delete é dele.
    e.teclar(cx, "s");
    clicar_na_foto(&e, cx, [0.7, 0.7], Modifiers::default());
    e.teclar(cx, "delete");
    assert!(retoques(&e, cx).is_empty());
    assert_eq!(camadas(&e, cx).len(), 1, "a máscara fica");
}

/// ➖ **"Subtrair" não passa para outra máscara.** A máscara nova nascia de
/// um traço que subtrai — e não mostrava nada.
#[gpui_kit::test]
fn subtrair_nao_passa_para_outra_mascara(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    clicar_no_painel(&e, cx, "local-modo-1");
    assert!(vista(&e, cx).subtrair, "o segmento liga o Subtrair");
    arrastar(&e, cx, &[[0.32, 0.3], [0.32, 0.5]], Modifiers::default());
    assert_eq!(camadas(&e, cx)[0].componentes[1].modo, Modo::Subtrair);

    // Larga a máscara e cria outra pela ferramenta.
    e.teclar(cx, "escape");
    e.teclar(cx, "escape");
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.7, 0.7], [0.8, 0.7]], Modifiers::default());
    let c = camadas(&e, cx);
    assert_eq!(c.len(), 2);
    assert_eq!(
        c[1].componentes[0].modo,
        Modo::Somar,
        "a máscara nova começa somando"
    );
    assert!(!vista(&e, cx).subtrair);

    // Escolher a 1ª na lista também começa somando.
    clicar_no_painel(&e, cx, "local-modo-1");
    clicar_no_painel(&e, cx, "mascara-0");
    assert!(!vista(&e, cx).subtrair, "trocar de máscara volta a somar");
}

/// ➕ **O "+" é um botão**: cria a máscara nova com a ferramenta de máscara
/// que está na mão (o pincel, se nenhuma) — o texto do painel manda usá-lo.
#[gpui_kit::test]
fn o_mais_cria_uma_mascara_nova(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.revelacao(cx, |tela, _w, cx| tela.abrir_o_painel_local(cx));
    clicar_no_painel(&e, cx, "nova-mascara");
    let v = vista(&e, cx);
    assert_eq!(
        v.ferramenta,
        Some(Ferramenta::Pincel),
        "sem ferramenta, o pincel"
    );
    assert!(v.criando);

    e.teclar(cx, "shift-m");
    arrastar(&e, cx, &[[0.5, 0.5], [0.6, 0.6]], Modifiers::default());
    assert_eq!(vista(&e, cx).mascara_sel, Some(0));
    clicar_no_painel(&e, cx, "nova-mascara");
    let v = vista(&e, cx);
    assert_eq!(v.ferramenta, Some(Ferramenta::Radial), "fica a da mão");
    assert_eq!(v.mascara_sel, None);
    assert!(v.criando);
    arrastar(&e, cx, &[[0.2, 0.2], [0.3, 0.3]], Modifiers::default());
    assert_eq!(camadas(&e, cx).len(), 2, "o gesto seguinte é outra máscara");
}

/// 📐 **Linear e radial pelas alças**: a borda do radial estica, e um clique
/// na alça sem arrastar não vira passo do histórico.
#[gpui_kit::test]
fn linear_e_radial_pelas_alcas(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "shift-m");
    arrastar(
        &e,
        cx,
        &[[0.5, 0.5], [0.55, 0.55], [0.6, 0.6]],
        Modifiers::default(),
    );
    let raio_de = |c: &[revelacao_core::locais::Camada]| match &c[0].componentes[0].forma {
        Forma::Radial(g) => (g.centro, g.raio_x),
        outra => panic!("um radial, não {outra:?}"),
    };
    let (centro, raio) = raio_de(&camadas(&e, cx));
    assert!((centro[0] - 0.5).abs() < 0.01);

    // A alça do centro: o clique sem arrastar não muda nada.
    clicar_na_foto(&e, cx, centro, Modifiers::default());
    assert_eq!(camadas(&e, cx)[0].componentes.len(), 1, "não cria outro");
    assert_eq!(raio_de(&camadas(&e, cx)), (centro, raio));

    // A alça da borda (RaioX): à direita do centro, a `raio` do maior lado.
    let (w, h) = e.revelacao(cx, |tela, _w, _cx| {
        tela.tamanho_da_copia_local().expect("medida")
    });
    let borda = [centro[0] + raio * w.max(h) / w, centro[1]];
    let longe = [borda[0] + 0.1, borda[1]];
    arrastar(
        &e,
        cx,
        &[borda, [borda[0] + 0.05, borda[1]], longe],
        Modifiers::default(),
    );
    let (_, maior) = raio_de(&camadas(&e, cx));
    assert!(maior > raio * 1.2, "a borda estica: {raio} → {maior}");

    // Dois ⌘Z: a borda, e depois o radial — o clique não deixou passo.
    e.teclar(cx, "cmd-z");
    assert_eq!(raio_de(&camadas(&e, cx)).1, raio);
    e.teclar(cx, "cmd-z");
    assert!(camadas(&e, cx).is_empty(), "o clique na alça não foi passo");
}

/// ➰ **O laço livre fecha ao soltar; o poligonal, no primeiro ponto; com
/// menos de três pontos não há laço.**
#[gpui_kit::test]
fn o_laco_livre_e_o_poligonal(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "l");
    arrastar(&e, cx, &[[0.2, 0.2], [0.3, 0.2]], Modifiers::default());
    assert!(camadas(&e, cx).is_empty(), "dois pontos não fazem laço");
    arrastar(
        &e,
        cx,
        &[[0.2, 0.2], [0.4, 0.2], [0.4, 0.4], [0.2, 0.4]],
        Modifiers::default(),
    );
    assert_eq!(camadas(&e, cx).len(), 1, "soltar fecha o laço livre");

    clicar_no_painel(&e, cx, "local-laco-1");
    for q in [[0.6, 0.6], [0.8, 0.6], [0.8, 0.8]] {
        clicar_na_foto(&e, cx, q, Modifiers::default());
    }
    assert_eq!(vista(&e, cx).poligono, 3);
    // Trocar para Livre e voltar larga os vértices.
    clicar_no_painel(&e, cx, "local-laco-0");
    clicar_no_painel(&e, cx, "local-laco-1");
    assert_eq!(vista(&e, cx).poligono, 0, "trocar o modo larga o laço");
    for q in [[0.6, 0.6], [0.8, 0.6], [0.8, 0.8], [0.6, 0.6]] {
        clicar_na_foto(&e, cx, q, Modifiers::default());
    }
    let c = camadas(&e, cx);
    assert_eq!(vista(&e, cx).poligono, 0, "o primeiro ponto fecha");
    assert_eq!(c[0].componentes.len(), 2, "no laço da máscara escolhida");
    assert!(matches!(&c[0].componentes[1].forma, Forma::Laco(l) if l.pontos.len() == 3));
}

/// 👁️ **Olho, inverter e lixeira da máscara não mexem na escolhida**; tirar o
/// último componente apaga a máscara.
#[gpui_kit::test]
fn os_botoes_da_mascara_nao_mexem_na_escolhida(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    clicar_no_painel(&e, cx, "nova-mascara-pincel");
    arrastar(&e, cx, &[[0.6, 0.6], [0.7, 0.6]], Modifiers::default());
    clicar_no_painel(&e, cx, "mascara-0");
    assert_eq!(vista(&e, cx).mascara_sel, Some(0));

    clicar_no_painel(&e, cx, "ver-1");
    clicar_no_painel(&e, cx, "inverter-1");
    let c = camadas(&e, cx);
    assert!(!c[1].visivel && c[1].invertida);
    assert_eq!(
        vista(&e, cx).mascara_sel,
        Some(0),
        "olho e inverter não escolhem"
    );
    clicar_no_painel(&e, cx, "excluir-1");
    assert_eq!(camadas(&e, cx).len(), 1);
    assert_eq!(
        vista(&e, cx).mascara_sel,
        Some(0),
        "a lixeira da outra não escolhe"
    );

    clicar_no_painel(&e, cx, "tirar-0-0");
    assert!(camadas(&e, cx).is_empty(), "sem componente, a máscara sai");
    assert_eq!(vista(&e, cx).mascara_sel, None);
}

/// 🩹 **Carimbo e band-aid como no Lightroom**: o clique acha a origem e já
/// deixa o retoque escolhido; ⌥-clique fixa a origem; arrastar o branco move
/// o destino (a origem fica); Delete apaga.
#[gpui_kit::test]
fn carimbo_e_band_aid_do_jeito_do_lightroom(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "s");
    clicar_na_foto(&e, cx, [0.3, 0.5], Modifiers::default());
    let r = retoques(&e, cx);
    let c = r[0].carimbo().expect("um carimbo");
    assert!(c.origem != [0.3, 0.5], "a origem é escolhida ao lado");
    assert_eq!(
        vista(&e, cx).selecionado,
        Some(0),
        "e o retoque fica escolhido"
    );

    e.teclar(cx, "escape");
    clicar_na_foto(&e, cx, [0.7, 0.3], alt());
    assert_eq!(retoques(&e, cx).len(), 1, "o ⌥-clique não cria retoque");
    clicar_na_foto(&e, cx, [0.3, 0.8], Modifiers::default());
    let r = retoques(&e, cx);
    let c = r[1].carimbo().expect("outro carimbo");
    assert!(
        (c.origem[0] - 0.7).abs() < 0.01 && (c.origem[1] - 0.3).abs() < 0.01,
        "com a origem do ⌥-clique: {:?}",
        c.origem
    );

    // O destino arrastado anda; a origem fica onde estava.
    arrastar(
        &e,
        cx,
        &[[0.3, 0.8], [0.33, 0.8], [0.35, 0.8]],
        Modifiers::default(),
    );
    let r = retoques(&e, cx);
    let c = r[1].carimbo().expect("o carimbo");
    assert!((c.destino_inicial[0] - 0.35).abs() < 0.01, "o destino anda");
    assert!((c.origem[0] - 0.7).abs() < 0.01, "a origem fica");

    e.teclar(cx, "delete");
    assert_eq!(retoques(&e, cx).len(), 1, "Delete apaga o escolhido");

    e.teclar(cx, "j");
    clicar_na_foto(&e, cx, [0.6, 0.6], Modifiers::default());
    assert!(matches!(retoques(&e, cx).last(), Some(Retoque::Heal(_))));
}

/// ✨ **O Content-Aware pinta e cerca**; cercar com menos de três pontos não
/// faz nada.
#[gpui_kit::test]
fn o_content_aware_pinta_e_cerca(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "shift-j");
    arrastar(&e, cx, &[[0.3, 0.3], [0.35, 0.3]], Modifiers::default());
    assert!(matches!(
        retoques(&e, cx).as_slice(),
        [Retoque::Preencher(p)] if p.laco.is_empty() && p.caminho.len() >= 2
    ));

    clicar_no_painel(&e, cx, "local-area-1");
    arrastar(&e, cx, &[[0.6, 0.6], [0.65, 0.6]], Modifiers::default());
    assert_eq!(retoques(&e, cx).len(), 1, "dois pontos não cercam");
    arrastar(
        &e,
        cx,
        &[[0.6, 0.6], [0.7, 0.6], [0.7, 0.7], [0.6, 0.7]],
        Modifiers::default(),
    );
    assert!(matches!(
        retoques(&e, cx).last(),
        Some(Retoque::Preencher(p)) if p.laco.len() >= 3
    ));
}

/// 🎛️ **Os sliders e os colchetes mexem na ferramenta e no retoque
/// escolhido, num passo só**; a lista de retoques escolhe e acerta os
/// sliders.
#[gpui_kit::test]
fn sliders_colchetes_e_a_lista_de_retoques(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "s");
    clicar_na_foto(&e, cx, [0.3, 0.5], Modifiers::default());
    let raio = retoques(&e, cx)[0].traco().raio;
    e.teclar(cx, "]");
    let maior = retoques(&e, cx)[0].traco().raio;
    assert!(
        maior > raio,
        "] cresce o retoque escolhido: {raio} → {maior}"
    );
    e.teclar(cx, "}");
    let v = vista(&e, cx);
    let feather = retoques(&e, cx)[0].traco().feather;
    assert!(
        (feather - v.feather).abs() < 1e-4,
        "}} muda a suavização dele"
    );
    assert!(
        (v.trilho_da_suavizacao - v.feather).abs() < 1e-3,
        "e o trilho"
    );
    e.teclar(cx, "cmd-z");
    e.teclar(cx, "cmd-z");
    assert!(
        (retoques(&e, cx)[0].traco().raio - raio).abs() < 1e-6,
        "um passo cada"
    );

    // Um segundo retoque, longe; a lista volta ao primeiro.
    clicar_na_foto(&e, cx, [0.75, 0.5], Modifiers::default());
    e.teclar(cx, "escape");
    e.teclar(cx, "escape");
    assert_eq!(vista(&e, cx).ferramenta, None);
    e.revelacao(cx, |tela, _w, cx| tela.abrir_o_painel_local(cx));
    clicar_no_painel(&e, cx, "retoque-0");
    let v = vista(&e, cx);
    assert_eq!(v.selecionado, Some(0), "a linha escolhe");
    assert_eq!(
        v.ferramenta,
        Some(Ferramenta::Carimbo),
        "com a ferramenta dele"
    );
    let r = &retoques(&e, cx)[0];
    assert!((v.trilho_da_opacidade - r.opacidade()).abs() < 1e-3);
    assert!((v.trilho_da_suavizacao - r.traco().feather).abs() < 1e-3);
}

/// 📍 **H esconde e mostra as marcações**.
#[gpui_kit::test]
fn h_esconde_e_mostra_as_marcacoes(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "s");
    for x in [0.3, 0.6] {
        clicar_na_foto(&e, cx, [x, 0.5], Modifiers::default());
    }
    let com = vista(&e, cx).marcas;
    e.teclar(cx, "h");
    let v = vista(&e, cx);
    assert!(!v.marcacoes);
    assert!(v.marcas < com, "H esconde: {com} → {}", v.marcas);
    e.teclar(cx, "h");
    assert_eq!(vista(&e, cx).marcas, com, "H de novo mostra");
}

/// ↩️ **Depois do ⌘Z a seleção não aponta para o que sumiu**, e o traço
/// seguinte não se perde.
#[gpui_kit::test]
fn desfazer_nao_deixa_selecao_orfa(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    e.teclar(cx, "cmd-z");
    assert!(camadas(&e, cx).is_empty());
    assert_eq!(
        vista(&e, cx).mascara_sel,
        None,
        "a máscara desfeita sai da seleção"
    );
    arrastar(&e, cx, &[[0.3, 0.6], [0.4, 0.6]], Modifiers::default());
    assert_eq!(camadas(&e, cx).len(), 1, "o traço seguinte vira máscara");

    e.teclar(cx, "s");
    clicar_na_foto(&e, cx, [0.7, 0.7], Modifiers::default());
    assert_eq!(vista(&e, cx).selecionado, Some(0));
    e.teclar(cx, "cmd-z");
    assert_eq!(vista(&e, cx).selecionado, None, "nem o retoque desfeito");
    e.teclar(cx, "delete");
    assert_eq!(
        camadas(&e, cx).len(),
        1,
        "e o Delete não apaga o que não vê"
    );
}

/// 🔁 **O duplo clique sem ferramenta entra no retoque**, com a ferramenta
/// dele e o zoom de antes.
#[gpui_kit::test]
fn o_duplo_clique_entra_no_retoque(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "j");
    clicar_na_foto(&e, cx, [0.4, 0.4], Modifiers::default());
    e.teclar(cx, "escape");
    e.teclar(cx, "escape");
    assert_eq!(vista(&e, cx).ferramenta, None);
    let escala = e.revelacao(cx, |tela, _w, _cx| tela.escala_local());
    duplo_clique_na_foto(&e, cx, [0.4, 0.4]);
    let v = vista(&e, cx);
    assert_eq!(v.ferramenta, Some(Ferramenta::BandAid));
    assert_eq!(v.selecionado, Some(0));
    assert_eq!(
        e.revelacao(cx, |tela, _w, _cx| tela.escala_local()),
        escala,
        "o zoom volta ao de antes do par de cliques"
    );
}

/// 🔒 **Foto travada: nada da Revelação local responde** — nem a tecla, nem
/// o botão, nem o gesto.
#[gpui_kit::test]
fn foto_travada_nao_aceita_revelacao_local(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.revelacao(cx, |tela, _w, cx| {
        tela.travar_a_revelacao_local(cx);
        tela.abrir_o_painel_local(cx);
    });
    e.teclar(cx, "k");
    assert_eq!(
        vista(&e, cx).ferramenta,
        None,
        "a tecla não pega a ferramenta"
    );
    clicar_no_painel(&e, cx, "local-carimbo");
    assert_eq!(vista(&e, cx).ferramenta, None, "nem o botão");
    assert!(camadas(&e, cx).is_empty() && retoques(&e, cx).is_empty());
}

/// 🖼️ **Trocar de foto larga a seleção**: a máscara escolhida numa não fica
/// escolhida na outra.
#[gpui_kit::test]
fn trocar_de_foto_larga_a_selecao(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    assert_eq!(vista(&e, cx).mascara_sel, Some(0));
    e.revelacao(cx, |tela, window, cx| {
        let seguinte = (tela.posicao() + 1) % tela.acervo().len();
        tela.ir_para(seguinte, window, cx);
    });
    e.esperar(cx);
    let v = vista(&e, cx);
    assert_eq!(v.mascara_sel, None, "a seleção não atravessa de foto");
    assert!(
        v.criando,
        "com o pincel na mão, o traço seguinte é máscara nova"
    );
    assert!(camadas(&e, cx).is_empty(), "a outra foto não tem máscara");
}

// ---------------------------------------------------------------------------
// 🧪 Segunda rodada (dono, 03/out/2026: *"teste ainda mais essa ferramenta"*):
// os cantos — o meio do arrasto, o clique sem arrasto, a máscara oculta, a
// troca de foto, o Enquadrar, a mão, os sliders sobre o que está escolhido.
// ---------------------------------------------------------------------------

/// Aperta em `de` e arrasta até `ate`, **sem soltar**.
fn apertar_e_arrastar(e: &Estudio, cx: &mut TestAppContext, de: [f32; 2], ate: [f32; 2]) {
    let (a, b) = (na_foto(e, cx, de), na_foto(e, cx, ate));
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_mouse_move(a, None, Modifiers::default());
    visual.simulate_mouse_down(a, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
    visual.simulate_mouse_move(b, Some(MouseButton::Left), Modifiers::default());
    visual.run_until_parked();
}

fn soltar_em(e: &Estudio, cx: &mut TestAppContext, q: [f32; 2]) {
    let p = na_foto(e, cx, q);
    let mut visual = VisualTestContext::from_window(e.raiz.into(), cx);
    visual.simulate_mouse_up(p, MouseButton::Left, Modifiers::default());
    visual.run_until_parked();
}

/// ⎋ **O Esc no meio do arrasto desiste do traço** — como no Lightroom. O
/// gesto ficava pela metade: a ferramenta saía da mão e o traço provisório
/// continuava na foto, sem ninguém para soltá-lo.
#[gpui_kit::test]
fn esc_no_meio_do_arrasto_desiste_do_traco(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    apertar_e_arrastar(&e, cx, [0.3, 0.4], [0.4, 0.4]);
    assert!(e.revelacao(cx, |tela, _w, _cx| tela.gesto_local_em_curso()));
    e.teclar(cx, "escape");
    soltar_em(&e, cx, [0.4, 0.4]);
    let v = vista(&e, cx);
    assert!(
        !e.revelacao(cx, |tela, _w, _cx| tela.gesto_local_em_curso()),
        "o gesto não fica preso"
    );
    assert!(camadas(&e, cx).is_empty(), "o traço foi desistido");
    assert_eq!(
        v.ferramenta,
        Some(Ferramenta::Pincel),
        "o Esc gasto no traço não larga a ferramenta"
    );
    assert_eq!(tela_do_app(&e, cx), Tela::Revelacao);
}

/// ↩️ **O ⌘Z no meio do arrasto desiste do traço em curso** — e não desfaz o
/// anterior por baixo dele.
#[gpui_kit::test]
fn desfazer_no_meio_do_arrasto_desiste_do_traco(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    apertar_e_arrastar(&e, cx, [0.3, 0.6], [0.4, 0.6]);
    e.teclar(cx, "cmd-z");
    soltar_em(&e, cx, [0.4, 0.6]);
    assert!(!e.revelacao(cx, |tela, _w, _cx| tela.gesto_local_em_curso()));
    let c = camadas(&e, cx);
    assert_eq!(c.len(), 1, "o traço de antes fica");
    assert_eq!(c[0].componentes.len(), 1, "o do meio do ⌘Z não entra");
}

/// 👆 **Um clique sem arrastar com o linear ou o radial não cria máscara** —
/// nascia uma máscara de borda dura (o linear de 0,1%) ou um ponto (o radial
/// de 0,5%), que ninguém pediu. O pincel continua: um toque é uma pincelada.
#[gpui_kit::test]
fn clique_sem_arrasto_do_gradiente_nao_cria_mascara(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "m");
    clicar_na_foto(&e, cx, [0.5, 0.5], Modifiers::default());
    assert!(camadas(&e, cx).is_empty(), "o clique do linear não cria");
    e.teclar(cx, "shift-m");
    clicar_na_foto(&e, cx, [0.5, 0.5], Modifiers::default());
    assert!(camadas(&e, cx).is_empty(), "nem o do radial");
    e.teclar(cx, "k");
    clicar_na_foto(&e, cx, [0.5, 0.5], Modifiers::default());
    assert_eq!(camadas(&e, cx).len(), 1, "o toque do pincel pinta");
}

/// ⌥ **O ⌥ no primeiro traço não cria uma máscara que só subtrai** — ela
/// nascia vazia e não mostrava nada.
#[gpui_kit::test]
fn alt_no_primeiro_traco_cria_mascara_que_soma(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], alt());
    let c = camadas(&e, cx);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].componentes[0].modo, Modo::Somar);
    // Na máscara que já tem algo, o ⌥ subtrai.
    arrastar(&e, cx, &[[0.35, 0.3], [0.35, 0.5]], alt());
    assert_eq!(camadas(&e, cx)[0].componentes[1].modo, Modo::Subtrair);
}

/// 👁️ **Pintar na máscara oculta a mostra** — o traço ia para uma máscara
/// invisível, e na foto nada acontecia.
#[gpui_kit::test]
fn pintar_na_mascara_oculta_a_mostra(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    clicar_no_painel(&e, cx, "ver-0");
    assert!(!camadas(&e, cx)[0].visivel);
    assert_eq!(vista(&e, cx).mascara_sel, Some(0));
    arrastar(&e, cx, &[[0.3, 0.6], [0.4, 0.6]], Modifiers::default());
    let c = camadas(&e, cx);
    assert_eq!(c[0].componentes.len(), 2);
    assert!(c[0].visivel, "o traço novo mostra a máscara");
}

/// 💾 **A Revelação local fica na foto**: troca de foto e volta, e as
/// máscaras e os retoques estão lá.
#[gpui_kit::test]
fn a_revelacao_local_fica_na_foto_ao_trocar_e_voltar(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    arrastar(&e, cx, &[[0.3, 0.4], [0.4, 0.4]], Modifiers::default());
    e.teclar(cx, "s");
    clicar_na_foto(&e, cx, [0.7, 0.6], Modifiers::default());
    let (antes_c, antes_r) = (camadas(&e, cx), retoques(&e, cx));
    let aqui = e.revelacao(cx, |tela, _w, _cx| tela.posicao());
    e.revelacao(cx, |tela, window, cx| {
        let outra = (aqui + 1) % tela.acervo().len();
        tela.ir_para(outra, window, cx);
    });
    e.esperar(cx);
    assert!(camadas(&e, cx).is_empty(), "a outra foto não tem máscara");
    e.revelacao(cx, |tela, window, cx| tela.ir_para(aqui, window, cx));
    e.esperar(cx);
    assert_eq!(camadas(&e, cx), antes_c, "as máscaras voltam");
    assert_eq!(retoques(&e, cx), antes_r, "e os retoques");
}

/// ✂️ **Entrar no Enquadrar larga a ferramenta local** — o círculo, os
/// alfinetes e os contornos ficavam desenhados por cima do retângulo do corte.
#[gpui_kit::test]
fn o_enquadrar_larga_a_ferramenta_local(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "s");
    clicar_na_foto(&e, cx, [0.6, 0.6], Modifiers::default());
    assert!(vista(&e, cx).marcas > 0);
    e.teclar(cx, "r");
    assert!(
        e.revelacao(cx, |tela, _w, _cx| tela.cortando()),
        "R abre o Enquadrar"
    );
    let v = vista(&e, cx);
    assert_eq!(v.ferramenta, None, "a ferramenta sai da mão");
    assert_eq!(v.selecionado, None);
    assert!(
        e.revelacao(cx, |tela, _w, cx| tela.marcacoes_locais_para_teste(cx)),
        "nada da Revelação local por cima do corte"
    );
    e.teclar(cx, "s");
    assert_eq!(
        vista(&e, cx).ferramenta,
        None,
        "nem pela tecla, no Enquadrar"
    );
    assert_eq!(retoques(&e, cx).len(), 1, "o retoque fica");
}

/// ✋ **Espaço segurado é a mão, mesmo com a ferramenta na mão** — arrasta a
/// foto ampliada e não pinta.
#[gpui_kit::test]
fn espaco_segurado_e_a_mao_com_a_ferramenta(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    for _ in 0..3 {
        e.revelacao(cx, |tela, _w, cx| tela.passo_de_zoom(1, cx));
    }
    e.teclar(cx, "k");
    let meio_antes = na_foto(&e, cx, [0.5, 0.5]);
    e.teclar(cx, "space");
    arrastar(
        &e,
        cx,
        &[[0.5, 0.5], [0.52, 0.5], [0.55, 0.5]],
        Modifiers::default(),
    );
    e.soltar(cx, "space");
    assert!(
        camadas(&e, cx).is_empty(),
        "com o Espaço, o arrasto não pinta"
    );
    let meio_depois = na_foto(&e, cx, [0.5, 0.5]);
    assert!(
        (meio_depois.x - meio_antes.x).abs() > gpui_kit::px(5.),
        "a foto andou: {meio_antes:?} → {meio_depois:?}"
    );
    assert_eq!(vista(&e, cx).ferramenta, Some(Ferramenta::Pincel));
    arrastar(&e, cx, &[[0.5, 0.5], [0.52, 0.5]], Modifiers::default());
    assert_eq!(camadas(&e, cx).len(), 1, "soltou o Espaço, o pincel pinta");
}

/// 🎚️ **A Suavização muda o radial escolhido** — como no Lightroom: com a
/// máscara de um radial escolhida e o radial na mão, o slider é dele, num
/// passo só. Antes ele só valia para o próximo, e o radial na foto não mudava.
#[gpui_kit::test]
fn a_suavizacao_muda_o_radial_escolhido(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "shift-m");
    arrastar(&e, cx, &[[0.5, 0.5], [0.6, 0.6]], Modifiers::default());
    let feather =
        |e: &Estudio, cx: &mut TestAppContext| match &camadas(e, cx)[0].componentes[0].forma {
            Forma::Radial(g) => g.feather,
            outra => panic!("um radial, não {outra:?}"),
        };
    let antes = feather(&e, cx);
    e.revelacao(cx, |tela, _w, cx| tela.soltar_o_slider_local(1, 0.9, cx));
    assert!(
        (feather(&e, cx) - 0.9).abs() < 1e-4,
        "o radial escolhido muda"
    );
    e.teclar(cx, "cmd-z");
    assert!((feather(&e, cx) - antes).abs() < 1e-4, "um ⌘Z volta");
    // Escolher a máscara põe o trilho na suavização dela.
    e.teclar(cx, "escape");
    e.teclar(cx, "escape");
    e.revelacao(cx, |tela, _w, cx| tela.soltar_o_slider_local(1, 0.1, cx));
    clicar_no_painel(&e, cx, "mascara-0");
    assert!(
        (vista(&e, cx).trilho_da_suavizacao - antes).abs() < 1e-3,
        "o trilho mostra a do radial escolhido"
    );
}

/// 🎚️ **Opacidade e Tamanho mudam o retoque escolhido, num passo cada.**
#[gpui_kit::test]
fn opacidade_e_tamanho_mudam_o_retoque_escolhido(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "s");
    clicar_na_foto(&e, cx, [0.5, 0.5], Modifiers::default());
    let raio = retoques(&e, cx)[0].traco().raio;
    e.revelacao(cx, |tela, _w, cx| tela.soltar_o_slider_local(2, 0.4, cx));
    assert!((retoques(&e, cx)[0].opacidade() - 0.4).abs() < 1e-4);
    e.revelacao(cx, |tela, _w, cx| tela.soltar_o_slider_local(0, 0.8, cx));
    assert!(
        retoques(&e, cx)[0].traco().raio > raio,
        "o Tamanho cresce o retoque"
    );
    e.teclar(cx, "cmd-z");
    assert!((retoques(&e, cx)[0].traco().raio - raio).abs() < 1e-6);
    e.teclar(cx, "cmd-z");
    assert!((retoques(&e, cx)[0].opacidade() - 1.0).abs() < 1e-4);
    assert_eq!(retoques(&e, cx).len(), 1, "os dois ⌘Z não levam o retoque");
}

/// 🩹 **Mover o retoque é um passo**: o ⌘Z o devolve ao lugar, e o seguinte
/// tira o retoque.
#[gpui_kit::test]
fn mover_o_retoque_e_um_passo(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "j");
    clicar_na_foto(&e, cx, [0.4, 0.5], Modifiers::default());
    arrastar(
        &e,
        cx,
        &[[0.4, 0.5], [0.43, 0.5], [0.45, 0.52]],
        Modifiers::default(),
    );
    let destino = |e: &Estudio, cx: &mut TestAppContext| {
        retoques(e, cx)[0]
            .carimbo()
            .expect("band-aid")
            .destino_inicial
    };
    assert!((destino(&e, cx)[0] - 0.45).abs() < 0.01);
    e.teclar(cx, "cmd-z");
    assert!((destino(&e, cx)[0] - 0.4).abs() < 0.01, "o ⌘Z o devolve");
    e.teclar(cx, "cmd-z");
    assert!(retoques(&e, cx).is_empty());
}

/// ✨ **O Content-Aware cerca também no poligonal.**
#[gpui_kit::test]
fn o_content_aware_cerca_no_poligonal(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "shift-j");
    clicar_no_painel(&e, cx, "local-area-1");
    clicar_no_painel(&e, cx, "local-laco-1");
    for q in [[0.6, 0.6], [0.7, 0.6], [0.7, 0.7], [0.6, 0.6]] {
        clicar_na_foto(&e, cx, q, Modifiers::default());
    }
    assert!(matches!(
        retoques(&e, cx).as_slice(),
        [Retoque::Preencher(p)] if p.laco.len() == 3
    ));
}

/// 🔀 **Trocar de ferramenta no meio do arrasto desiste do traço** — e o
/// soltar depois não cria nada com a ferramenta nova.
#[gpui_kit::test]
fn trocar_de_ferramenta_no_meio_do_arrasto(cx: &mut TestAppContext) {
    let e = na_foto_a(cx);
    e.teclar(cx, "k");
    apertar_e_arrastar(&e, cx, [0.3, 0.4], [0.4, 0.4]);
    e.teclar(cx, "m");
    soltar_em(&e, cx, [0.4, 0.4]);
    assert!(!e.revelacao(cx, |tela, _w, _cx| tela.gesto_local_em_curso()));
    assert!(camadas(&e, cx).is_empty(), "nem o traço nem um linear");
    assert_eq!(vista(&e, cx).ferramenta, Some(Ferramenta::Linear));
}
