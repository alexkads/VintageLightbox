//! O estresse da Revelação: a tira com milhares de fotos, milhares de eventos
//! de slider num gesto só, o Enquadrar arrastado sem parar e o zoom com o bruto
//! chegando fora de ordem.
//!
//! Mora aqui dentro, e não em `crate::estresse`, porque o que ele confere é
//! estado da tela — o palco, o histórico, o cache da tira — que só os módulos
//! filhos enxergam. Ver o cabeçalho de `crate::estresse` para como rodar.
//!
//! 🚨 **Só o `GravadorDeMentira`**: nada daqui toca banco nem site.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{px, Bounds, TestAppContext};
use image::{DynamicImage, Rgb, RgbImage};
use tempfile::TempDir;

use super::super::lightroom::mentira::EscolhaDeMentira;
use super::super::persistencia::mentira::GravadorDeMentira;
use super::super::presets::mentira::GuardaDeMentira;
use super::tira::FILTROS_DA_TIRA;
use super::*;
use crate::biblioteca::miniaturas::Miniatura;
use crate::estresse::{cronometrar, memoria_em_mb, relatar, Lcg};
use crate::revelacao::zoom::{self, Ponto};

fn previews_descartaveis() -> (Arc<PreviewManager>, TempDir) {
    let dir = TempDir::new().expect("diretório temporário");
    (
        Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf())),
        dir,
    )
}

/// Uma imagem com gradiente — com cor uniforme o JPEG fica pequeno demais para
/// medir alguma coisa.
fn imagem(largura: u32, altura: u32) -> DynamicImage {
    DynamicImage::ImageRgb8(RgbImage::from_fn(largura, altura, |x, y| {
        Rgb([(x % 256) as u8, (y % 256) as u8, ((x ^ y) % 256) as u8])
    }))
}

fn foto(i: usize) -> PhotoViewModel {
    PhotoViewModel {
        id: format!("id-{i:05}"),
        name: format!("DSC_{i:05}.jpg"),
        path: format!("/fotos/DSC_{i:05}.jpg"),
        rating: (i % 6) as i32,
        comprada: i.is_multiple_of(11),
        ..Default::default()
    }
}

fn janela(
    cx: &mut TestAppContext,
    previews: Arc<PreviewManager>,
    gravador: Arc<GravadorDeMentira>,
) -> gpui::WindowHandle<Revelacao> {
    cx.update(gpui_component::init);
    cx.add_window(move |window, cx| {
        Revelacao::nova(
            previews,
            gravador,
            Arc::new(GuardaDeMentira::default()),
            Arc::new(EscolhaDeMentira::default()),
            Vec::new(),
            window,
            cx,
        )
    })
}

fn passar_a_espera(cx: &mut TestAppContext) {
    cx.executor().advance_clock(ESPERA_DA_GRAVACAO * 2);
    cx.run_until_parked();
}

/// Quantas miniaturas das `raio` vizinhas da aberta (para cada lado) estão
/// prontas no cache da tira.
fn vizinhas_prontas(tela: &Revelacao, raio: usize) -> (usize, usize) {
    let posicao = tela.posicao;
    let de = posicao.saturating_sub(raio);
    let ate = (posicao + raio).min(tela.acervo.len() - 1);
    let prontas = (de..=ate)
        .filter(|i| {
            matches!(
                tela.miniaturas_da_tira.espiar(&tela.acervo[*i].id),
                Some(Miniatura::Pronta(_))
            )
        })
        .count();
    (prontas, ate - de + 1)
}

fn desenhar(visual: &mut gpui::VisualTestContext, largura: f32, altura: f32) {
    visual.draw(
        gpui::Point::default(),
        gpui::size(px(largura), px(altura)),
        |_window, _cx| gpui::Empty,
    );
}

/// Quanto custa um quadro da Revelação com a tira de `n` fotos.
fn medir_o_quadro(
    cx: &mut TestAppContext,
    janela: gpui::WindowHandle<Revelacao>,
    quadros: u32,
) -> Duration {
    let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
    desenhar(&mut visual, 2000., 1300.);
    visual.run_until_parked();
    let inicio = Instant::now();
    for _ in 0..quadros {
        janela
            .update(&mut visual, |_tela, _window, cx| cx.notify())
            .expect("a janela aberta");
        desenhar(&mut visual, 2000., 1300.);
    }
    inicio.elapsed() / quadros
}

/// 🚨 **A tira com 10.000 fotos: abrir, carregar, recortar, marcar e andar de
/// ponta a ponta.**
///
/// O cache da tira guarda 512 miniaturas. Com o acervo maior que isso, o que
/// fica guardado tem de ser **o que está perto do palco** — é para lá que a
/// seta vai e é o que o olho vê —, e o carregamento de cada troca tem de ser
/// pequeno, e não o acervo inteiro de novo.
#[gpui::test]
fn estresse_a_tira_com_dez_mil_fotos(cx: &mut TestAppContext) {
    const N: usize = 10_000;
    let (previews, _dir) = previews_descartaveis();
    let pequena = imagem(24, 16);
    cronometrar("semear 10.000 miniaturas", Duration::from_secs(60), || {
        for i in 0..N {
            previews
                .save_thumbnail(&format!("id-{i:05}"), &pequena)
                .expect("gravar miniatura");
        }
    });
    let memoria_antes = memoria_em_mb();
    let gravador = Arc::new(GravadorDeMentira::default());
    let janela = janela(cx, previews, gravador.clone());
    let acervo: Vec<PhotoViewModel> = (0..N).map(foto).collect();

    janela
        .update(cx, |tela, window, cx| {
            cronometrar(
                "abrir a Revelação no meio de 10.000",
                Duration::from_millis(500),
                || tela.abrir_no_acervo(acervo, N / 2, window, cx),
            );
        })
        .expect("a janela aberta");
    cronometrar("carregar a tira", Duration::from_secs(20), || {
        cx.run_until_parked()
    });

    janela
        .update(cx, |tela, _window, _cx| {
            let guardadas = tela.miniaturas_da_tira.quantas_na_memoria();
            assert!(guardadas <= MINIATURAS_DA_TIRA, "{guardadas} no cache");
            let (prontas, vizinhas) = vizinhas_prontas(tela, 100);
            assert_eq!(
                prontas, vizinhas,
                "as vizinhas do palco têm de estar no cache da tira"
            );
            assert!(
                tela.miniaturas_faltando().len() < 10,
                "com o palco parado, nada falta perto dele ({} faltando)",
                tela.miniaturas_faltando().len()
            );
        })
        .expect("a janela aberta");

    // Andar: 2.000 setas para a frente, cada uma com o carregamento dela.
    let passos = 2_000;
    let inicio = Instant::now();
    let mut pior = Duration::ZERO;
    for _ in 0..passos {
        let t = Instant::now();
        janela
            .update(cx, |tela, window, cx| tela.andar(1, window, cx))
            .expect("a janela aberta");
        cx.run_until_parked();
        pior = pior.max(t.elapsed());
    }
    let por_passo = inicio.elapsed() / passos;
    relatar(
        "uma seta na tira de 10.000 (média)",
        por_passo,
        Duration::from_millis(20),
    );
    relatar(
        "uma seta na tira de 10.000 (pior)",
        pior,
        Duration::from_millis(200),
    );
    assert!(por_passo <= Duration::from_millis(20));

    janela
        .update(cx, |tela, window, cx| {
            assert_eq!(tela.posicao(), N / 2 + passos as usize);
            let (prontas, vizinhas) = vizinhas_prontas(tela, 100);
            assert_eq!(prontas, vizinhas, "depois de andar, o cache seguiu o palco");
            assert!(tela.miniaturas_da_tira.quantas_na_memoria() <= MINIATURAS_DA_TIRA);

            // Os recortes, cada um sobre as 10.000.
            for (rotulo, filtro) in FILTROS_DA_TIRA {
                let tira = cronometrar(
                    &format!("recortar \"{rotulo}\""),
                    Duration::from_millis(200),
                    || {
                        tela.recortar(filtro, cx);
                        tela.na_tira()
                    },
                );
                assert!(
                    tira.contains(&tela.posicao()),
                    "a aberta nunca some da tira"
                );
                cronometrar(
                    &format!("⌘A em \"{rotulo}\""),
                    Duration::from_millis(200),
                    || tela.marcar_todas(cx),
                );
                assert!(tela.marcadas().len() >= tira.len());
                tela.desmarcar(cx);
                assert_eq!(tela.marcadas().len(), 1);
            }
            tela.recortar(biblioteca_core::acervo::Filtro::Todas, cx);

            // De ponta a ponta: a primeira e a última, pelo clique.
            cronometrar("ir à primeira", Duration::from_millis(200), || {
                tela.ir_para(0, window, cx)
            });
            for _ in 0..5 {
                tela.andar(-1, window, cx);
            }
            assert_eq!(tela.posicao(), 0, "a seta não dá a volta");
            cronometrar("ir à última", Duration::from_millis(200), || {
                tela.ir_para(N - 1, window, cx)
            });
            for _ in 0..5 {
                tela.andar(1, window, cx);
            }
            assert_eq!(tela.posicao(), N - 1, "a seta não dá a volta");
        })
        .expect("a janela aberta");
    cx.run_until_parked();

    // Andar não grava: nenhum slider foi tocado.
    assert!(gravador.gravado().is_empty(), "andar pela tira gravou algo");

    let quadro = medir_o_quadro(cx, janela, 5);
    relatar(
        "um quadro da Revelação com a tira de 10.000",
        quadro,
        Duration::from_millis(100),
    );
    if let (Some(antes), Some(depois)) = (memoria_antes, memoria_em_mb()) {
        println!("ℹ️ memória: {antes:.0} MB → {depois:.0} MB");
    }
    assert!(quadro <= Duration::from_millis(100));
}

/// 🔑 **O quadro da Revelação, por tamanho de tira.** Régua, não asserção de
/// fluidez: o perfil de teste não é o `--release`.
#[gpui::test]
#[ignore = "régua — rode com --ignored --nocapture"]
fn estresse_medir_o_quadro_por_tamanho_de_tira(cx: &mut TestAppContext) {
    let (previews, _dir) = previews_descartaveis();
    previews
        .save_preview("id-00000", &imagem(2560, 1707))
        .expect("gravar preview");
    for n in [125usize, 500, 2_000, 10_000] {
        let janela = janela(cx, previews.clone(), Arc::new(GravadorDeMentira::default()));
        janela
            .update(cx, |tela, window, cx| {
                tela.abrir_no_acervo((0..n).map(foto).collect(), 0, window, cx)
            })
            .expect("a janela aberta");
        cx.run_until_parked();
        let quadro = medir_o_quadro(cx, janela, 10);
        relatar(
            &format!("um quadro com a tira de {n}"),
            quadro,
            Duration::from_micros(16_700),
        );
    }
}

/// Emite o `Change` de um slider como o ponteiro emite — sem esperar.
///
/// ⚠️ **O assinante só recebe quando o `update` de fora fecha**: dentro do
/// mesmo `update`, o evento ainda não chegou aos `Ajustes`. Um teste que
/// mexe e troca de foto no mesmo `update` aplica o gesto à foto **nova** — o
/// que o app de verdade nunca faz, porque cada evento do sistema é um ciclo.
fn mexer(tela: &mut Revelacao, controle: usize, valor: f32, cx: &mut Context<Revelacao>) {
    let estado = tela.controles[controle].estado.clone();
    estado.update(cx, |_, cx| {
        cx.emit(SliderEvent::Change(
            gpui_component::slider::SliderValue::Single(valor),
        ));
    });
}

/// 🚨 **Mil eventos de slider por gesto, trinta gestos: trinta gravações e um
/// passo de histórico por gesto.**
///
/// É o arrasto de verdade num trackpad: centenas de `Change` num segundo, em
/// controles diferentes se o dedo escorrega. Uma gravação por evento seria um
/// `UPDATE` por milímetro; um passo por evento esvaziaria o `Cmd+Z` em meio
/// segundo.
#[gpui::test]
fn estresse_mil_eventos_de_slider_por_gesto(cx: &mut TestAppContext) {
    const GESTOS: usize = 30;
    const EVENTOS: usize = 1_000;
    let (previews, _dir) = previews_descartaveis();
    previews
        .save_preview("id-00001", &imagem(512, 341))
        .expect("gravar preview");
    previews
        .save_preview("id-00002", &imagem(300, 200))
        .expect("gravar preview");
    let gravador = Arc::new(GravadorDeMentira::default());
    let janela = janela(cx, previews, gravador.clone());
    janela
        .update(cx, |tela, window, cx| {
            tela.abrir_no_acervo(vec![foto(1), foto(2)], 0, window, cx)
        })
        .expect("a janela aberta");
    cx.run_until_parked();

    let mut g = Lcg::novo(53);
    let mut estados = Vec::new();
    let mut eventos = 0usize;
    let inicio = Instant::now();
    let mut entregando = Duration::ZERO;
    for _ in 0..GESTOS {
        let t = Instant::now();
        janela
            .update(cx, |tela, _window, cx| {
                let quantos = tela.controles.len();
                for _ in 0..EVENTOS {
                    // Nove em dez no mesmo controle; o resto escorrega.
                    let controle = if g.ate(10) == 0 {
                        g.ate(quantos)
                    } else {
                        eventos % quantos.min(7)
                    };
                    let d = tela.controles[controle].definicao;
                    mexer(tela, controle, g.entre(d.minimo, d.maximo), cx);
                    eventos += 1;
                }
            })
            .expect("a janela aberta");
        entregando += t.elapsed();
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(tela.pendente, "o gesto está aberto");
            })
            .expect("a janela aberta");
        // O executor anda um pouco no meio (o colher da GPU), sem fechar o gesto.
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        passar_a_espera(cx);
        janela
            .update(cx, |tela, _window, _cx| {
                assert!(!tela.pendente, "a espera fechou o gesto");
                estados.push(tela.estado());
            })
            .expect("a janela aberta");
    }
    let gasto = inicio.elapsed();
    relatar(
        &format!("{eventos} eventos de slider em {GESTOS} gestos"),
        gasto,
        Duration::from_secs(30),
    );
    let por_evento = entregando / eventos as u32;
    relatar(
        "um evento de slider (entregue, com o pedido à GPU)",
        por_evento,
        Duration::from_millis(2),
    );
    assert!(por_evento <= Duration::from_millis(2));

    let gravado = gravador.gravado();
    assert_eq!(gravado.len(), GESTOS, "uma gravação por gesto");
    for (i, (_, ajustes, _)) in gravado.iter().enumerate() {
        assert_eq!(
            *ajustes, estados[i].ajustes,
            "o gesto {i} gravou o que o dedo deixou"
        );
    }

    // O histórico: um passo por gesto, com o teto de 20. Cada `Cmd+Z` e cada
    // `Cmd+Shift+Z` também vai ao banco — são 19 + 19 gravações a mais.
    janela
        .update(cx, |tela, window, cx| {
            let mut desfeitos = 0;
            while tela.pode_desfazer() {
                tela.desfazer(window, cx);
                desfeitos += 1;
                assert!(desfeitos <= 20, "o histórico passou do teto");
            }
            assert_eq!(desfeitos, 19, "teto de 20: o mais antigo que sobrou");
            assert_eq!(
                tela.ajustes(),
                estados[GESTOS - 20].ajustes,
                "vinte passos para trás é o gesto que o teto guardou"
            );
            for _ in 0..19 {
                tela.refazer(window, cx);
            }
            assert_eq!(tela.ajustes(), estados[GESTOS - 1].ajustes);

            // Um gesto aberto e a seta no meio: grava a anterior antes.
            mexer(tela, 0, 2.5, cx);
        })
        .expect("a janela aberta");
    janela
        .update(cx, |tela, window, cx| {
            tela.andar(1, window, cx);
            assert_eq!(tela.posicao(), 1);
        })
        .expect("a janela aberta");
    cx.run_until_parked();
    let gravado = gravador.gravado();
    let depois_do_historico = GESTOS + 19 + 19;
    assert_eq!(gravado.len(), depois_do_historico + 1);
    let ultima = &gravado[depois_do_historico];
    assert_eq!(ultima.0, "id-00001", "a gravação é da foto que saiu");
    assert_eq!(ultima.1.exposure, 2.5);
    passar_a_espera(cx);
    assert_eq!(
        gravador.gravado().len(),
        depois_do_historico + 1,
        "a foto nova não herdou o gesto"
    );
}

/// 🚨 **A revelação atrasada da foto anterior não pode pintar a foto nova.**
///
/// Mexer num slider e apertar a seta antes de a GPU responder: o resultado
/// que volta é **da foto que saiu**. Colhê-lo depois da troca punha no palco
/// a imagem de uma foto sob o nome de outra — e numa foto nova sem ajuste
/// nenhum, ninguém pediria outra revelação para corrigir.
#[gpui::test]
fn estresse_a_revelacao_atrasada_nao_pinta_a_foto_nova(cx: &mut TestAppContext) {
    let (previews, _dir) = previews_descartaveis();
    // Tamanhos diferentes: é por eles que se sabe de quem é a imagem.
    previews
        .save_preview("id-00001", &imagem(640, 427))
        .expect("gravar preview");
    previews
        .save_preview("id-00002", &imagem(200, 300))
        .expect("gravar preview");
    let gravador = Arc::new(GravadorDeMentira::default());
    let janela = janela(cx, previews, gravador);
    janela
        .update(cx, |tela, window, cx| {
            tela.abrir_no_acervo(vec![foto(1), foto(2)], 0, window, cx)
        })
        .expect("a janela aberta");

    // A GPU abre numa thread própria: sem ela, não há o que atrasar.
    if !esperar_a_gpu(cx, janela) {
        println!("⚠️ sem GPU: não há revelação atrasada para conferir");
        return;
    }

    let mut trocas = 0;
    for rodada in 0..20 {
        janela
            .update(cx, |tela, window, cx| {
                // Na foto de 640×427, um ajuste…
                if tela.posicao() != 0 {
                    tela.andar(-1, window, cx);
                }
                mexer(tela, 0, 0.5 + rodada as f32 * 0.1, cx);
            })
            .expect("a janela aberta");
        janela
            .update(cx, |tela, window, cx| {
                assert!(tela.aguardando.is_some(), "o ajuste virou pedido à GPU");
                // …e a seta, antes da resposta.
                tela.andar(1, window, cx);
                assert_eq!(tela.foto_aberta().map(|f| f.id.as_str()), Some("id-00002"));
            })
            .expect("a janela aberta");
        // A GPU responde de verdade, fora do relógio do teste.
        std::thread::sleep(Duration::from_millis(150));
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        janela
            .update(cx, |tela, _window, _cx| {
                let aberta = tela.aberta.as_ref().expect("uma foto aberta");
                let revelada = aberta.revelada.as_ref().expect("a imagem da foto");
                assert_eq!(
                    (revelada.width(), revelada.height()),
                    (200, 300),
                    "rodada {rodada}: o palco da foto 2 mostra a revelação da foto 1"
                );
            })
            .expect("a janela aberta");
        trocas += 1;
    }
    println!("✅ {trocas} trocas com a GPU atrasada, e o palco sempre com a foto certa");
}

/// Espera a GPU abrir. `false` quando esta máquina não tem adaptador — e aí o
/// caso não tem o que conferir.
///
/// A thread do [`Processador`] abre o dispositivo em paralelo, e `disponivel()`
/// responde `None` enquanto isso: tratar `None` como "não tem" faria todo caso
/// que depende da GPU passar em branco, em toda máquina.
fn esperar_a_gpu(cx: &mut TestAppContext, janela: gpui::WindowHandle<Revelacao>) -> bool {
    let prazo = Instant::now() + Duration::from_secs(20);
    loop {
        let disponivel = janela
            .update(cx, |tela, _w, _cx| tela.processador.disponivel())
            .expect("a janela aberta");
        match disponivel {
            Some(true) => return true,
            Some(false) => return false,
            None if Instant::now() < prazo => std::thread::sleep(Duration::from_millis(20)),
            None => panic!("a GPU não respondeu em 20 s"),
        }
    }
}

/// Deixa a GPU responder de verdade e colhe. Devolve se a foto já está no palco.
fn deixar_a_gpu_responder(cx: &mut TestAppContext, janela: gpui::WindowHandle<Revelacao>) -> bool {
    std::thread::sleep(Duration::from_millis(2));
    cx.executor().advance_clock(INTERVALO_DE_COLHEITA * 2);
    cx.run_until_parked();
    janela
        .update(
            cx,
            |tela, _w, _cx| matches!(&tela.aberta, Some(aberta) if aberta.desenhada.is_some()),
        )
        .expect("a janela aberta")
}

/// Anda uma foto e espera ela aparecer.
///
/// Devolve `None` quando ela **já estava na tela** (cache ou antecipação), e
/// `Some(espera)` com o tempo que o palco ficou vazio quando foi preciso
/// esperar a GPU.
///
/// 🚨 **Afirma a regra no caminho**: em nenhum instante a foto crua vai ao
/// palco. Ou ela já está desenhada, ou não há nada.
///
/// ⚠️ **Quem responde "foi instantâneo?" é a prontidão, e não o relógio.**
/// `Instant::elapsed` nunca devolve zero — uma primeira versão deste caso
/// contava "esperas" por `!vazio.is_zero()` e acusava o cache de não servir
/// enquanto ele servia em 100% das setas.
fn esperar_a_foto(
    cx: &mut TestAppContext,
    janela: gpui::WindowHandle<Revelacao>,
    passo: i32,
    onde: &str,
) -> Option<Duration> {
    if passo != 0 {
        janela
            .update(cx, |tela, window, cx| tela.andar(passo, window, cx))
            .expect("a janela aberta");
    }

    let (pronta, esperada, nome) = janela
        .update(cx, |tela, _w, _cx| {
            let aberta = tela.aberta.as_ref().expect("uma foto aberta");
            assert!(
                aberta.bruta.is_some(),
                "{onde}: a crua sai da tela, não da memória — é o `\\`"
            );
            assert!(
                aberta.desenhada.is_some() || aberta.revelada.is_none(),
                "{onde}: a foto crua foi ao palco antes da revelação"
            );
            (
                aberta.desenhada.is_some(),
                aberta.origem.as_ref().map(|o| (o.largura, o.altura)),
                aberta.foto.name.clone(),
            )
        })
        .expect("a janela aberta");

    let comeco = Instant::now();
    if !pronta {
        let prazo = comeco + Duration::from_secs(10);
        while !deixar_a_gpu_responder(cx, janela) {
            assert!(
                Instant::now() < prazo,
                "{onde} ({nome}): o palco ficou vazio por 10 s"
            );
        }
    }
    let vazio = (!pronta).then(|| comeco.elapsed());

    janela
        .update(cx, |tela, _w, _cx| {
            let aberta = tela.aberta.as_ref().expect("uma foto aberta");
            let revelada = aberta.revelada.as_ref().expect("a revelação chegou");
            assert_eq!(
                Some((revelada.width(), revelada.height())),
                esperada,
                "{onde} ({nome}): o palco mostra a revelação de outra foto"
            );
        })
        .expect("a janela aberta");

    vazio
}

/// 🚨 **Percorrer uma tira de fotos reveladas não pode mostrar nenhuma delas
/// crua — e a volta tem de ser instantânea.**
///
/// Desde 17/set/2026 a foto com receita gravada **não** vai ao palco antes de o
/// motor responder (dono: *"primeiro mostra sem efeito e depois é aplicado a
/// receita"*). Isso troca um defeito visível por um custo: entre a seta e a
/// resposta da GPU o palco fica vazio. O cache de reveladas e a revelação
/// antecipada (`revelacao/cache.rs` e `antecipar_a_proxima`) existem para pagar
/// esse custo — *"precisa guardar um cache e fazer uma aplicação antecipada na
/// próxima foto pra garantir mais velocidade"* (dono, no mesmo dia).
///
/// Os testes de unidade provam as regras numa foto. O que só se vê aqui é o
/// **preço repetido**, em três travessias de quarenta fotos com a GPU de verdade
/// no meio:
///
/// 1. **ida a seco** — a seta apertada no instante em que a foto aparece. É o
///    pior caso: a antecipação não teve tempo de terminar, e o que se mede é a
///    ida à GPU inteira;
/// 2. **volta** — sobre fotos já reveladas. Aqui o cache tem de responder, e o
///    palco **não pode** ficar vazio nenhuma vez;
/// 3. **ida com pausa** — a seta apertada depois de um instante de folga, como
///    quem olha a foto antes de andar. É o caso real, e o que a antecipação
///    existe para resolver.
#[gpui::test]
fn estresse_percorrer_a_tira_de_fotos_reveladas(cx: &mut TestAppContext) {
    const FOTOS: usize = 40;

    let (previews, _dir) = previews_descartaveis();
    // Cada foto com um tamanho próprio: é por ele que se sabe que o que está no
    // palco é a revelação **desta** foto, e não a que ficou da anterior.
    let acervo: Vec<PhotoViewModel> = (0..FOTOS)
        .map(|i| {
            previews
                .save_preview(&format!("id-{i:05}"), &imagem(320 + i as u32 * 4, 240))
                .expect("gravar preview");
            PhotoViewModel {
                // Com receita gravada, senão não há revelação a esperar e o caso
                // não exercita nada.
                edit_exposure: Some(0.5 + i as f32 * 0.01),
                ..foto(i)
            }
        })
        .collect();

    let gravador = Arc::new(GravadorDeMentira::default());
    let janela = janela(cx, previews, gravador);
    janela
        .update(cx, |tela, window, cx| {
            tela.abrir_no_acervo(acervo, 0, window, cx)
        })
        .expect("a janela aberta");

    if !esperar_a_gpu(cx, janela) {
        println!("⚠️ sem GPU: não há revelação para esperar");
        return;
    }

    // ------------------------------------------------------------ 1. ida a seco
    let mut vazio_total = Duration::ZERO;
    let mut vazio_pior = Duration::ZERO;
    for passo in 0..FOTOS {
        let vazio = esperar_a_foto(
            cx,
            janela,
            if passo == 0 { 0 } else { 1 },
            &format!("ida a seco, passo {passo}"),
        )
        .unwrap_or_default();
        vazio_total += vazio;
        vazio_pior = vazio_pior.max(vazio);
    }
    relatar(
        &format!("ida a seco: palco vazio por seta, em {FOTOS} fotos (média)"),
        vazio_total / FOTOS as u32,
        Duration::from_millis(150),
    );
    relatar(
        "ida a seco: palco vazio por seta (pior)",
        vazio_pior,
        Duration::from_millis(600),
    );
    assert!(
        vazio_total / FOTOS as u32 <= Duration::from_millis(150),
        "a espera média pela revelação passou de 150 ms: a triagem pisca preto a cada seta"
    );
    assert!(
        vazio_pior <= Duration::from_millis(600),
        "uma das setas deixou o palco vazio por mais de 600 ms"
    );

    // ------------------------------------------------------------ 2. a volta
    let mut esperas_na_volta = 0;
    let volta = Instant::now();
    for passo in 1..FOTOS {
        if esperar_a_foto(cx, janela, -1, &format!("volta, passo {passo}")).is_some() {
            esperas_na_volta += 1;
        }
    }
    relatar(
        &format!(
            "volta por {} fotos já reveladas (o cache inteiro)",
            FOTOS - 1
        ),
        volta.elapsed(),
        Duration::from_millis(1500),
    );
    assert_eq!(
        esperas_na_volta, 0,
        "a volta pediu revelação de novo {esperas_na_volta} vezes — o cache não está servindo"
    );

    // ------------------------------------------------- 3. ida com pausa (o real)
    //
    // 🔑 **A pausa é o que a antecipação precisa.** Sem ela o caso mede só o
    // pior caso, e a seta apertada no milissegundo em que a foto aparece não é
    // como ninguém tria. Aqui a foto seguinte é revelada **antes** de a seta
    // chegar nela — e o palco não deve ficar vazio nenhuma vez.
    //
    // Estas fotos já estão no cache da ida, então o que se afirma é a soma do
    // cache com a antecipação: por qualquer um dos dois caminhos, instantâneo.
    let mut esperas_com_pausa = 0;
    for passo in 1..FOTOS {
        // A folga de quem olha a foto antes de andar.
        for _ in 0..5 {
            deixar_a_gpu_responder(cx, janela);
        }
        if esperar_a_foto(cx, janela, 1, &format!("ida com pausa, passo {passo}")).is_some() {
            esperas_com_pausa += 1;
        }
    }
    println!(
        "✅ ida com pausa: {} de {} setas instantâneas",
        FOTOS - 1 - esperas_com_pausa,
        FOTOS - 1
    );
    assert_eq!(
        esperas_com_pausa, 0,
        "{esperas_com_pausa} setas ainda esperaram a GPU com a folga de uma pausa"
    );
}

/// 🚨 **O Enquadrar arrastado sem parar: um passo e uma gravação por gesto, e o
/// retângulo sempre dentro da foto.**
///
/// Sessenta arrastos de alça com trezentos movimentos cada — muitos deles fora
/// do palco —, trocas de proporção no meio, o transferidor girando de ponta a
/// ponta e o slider do ângulo com mil eventos.
#[gpui::test]
fn estresse_o_enquadrar_arrastado_sem_parar(cx: &mut TestAppContext) {
    let (previews, _dir) = previews_descartaveis();
    // A cópia de trabalho de verdade: é nela que o endireitar gira a foto na CPU.
    previews
        .save_preview("id-00001", &imagem(2048, 1365))
        .expect("gravar preview");
    let gravador = Arc::new(GravadorDeMentira::default());
    let janela = janela(cx, previews, gravador.clone());

    let mut g = Lcg::novo(59);
    let mut esperadas = 0usize;
    let valido = |tela: &Revelacao| {
        let c = tela.corte_atual();
        for v in [
            c.crop_x(),
            c.crop_y(),
            c.crop_width(),
            c.crop_height(),
            c.angle(),
        ] {
            assert!(v.is_finite(), "NaN no corte: {c:?}");
        }
        assert!(c.crop_width() > 0. && c.crop_height() > 0., "{c:?}");
        assert!(c.crop_x() + c.crop_width() <= 1. + 1e-4, "{c:?}");
        assert!(c.crop_y() + c.crop_height() <= 1. + 1e-4, "{c:?}");
    };

    let movimentos_de_alca = janela
        .update(cx, |tela, window, cx| {
            tela.abrir(foto(1), window, cx);
            tela.palco = Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(1200.), px(800.)));
            tela.alternar_corte(window, cx);
            assert!(tela.cortando());

            let mut movimentos = 0usize;
            let inicio = Instant::now();
            for gesto in 0..60 {
                if gesto % 7 == 3 {
                    let p = corte::PROPORCOES[g.ate(corte::PROPORCOES.len())].1;
                    tela.travar_proporcao(p, cx);
                    // Travar é um gesto: grava — mesmo quando o retângulo já
                    // tinha a forma (uma escrita igual à anterior).
                    if p.is_some() {
                        esperadas += 1;
                    }
                    valido(tela);
                }
                let alca = {
                    let i = g.ate(corte::Alca::TODAS.len() + 1);
                    corte::Alca::TODAS.get(i).copied()
                };
                let x0 = g.entre(0., 1200.);
                let y0 = g.entre(0., 800.);
                tela.comecar_arrasto(alca, gpui::point(px(x0), px(y0)), cx);
                for _ in 0..300 {
                    let x = g.entre(-600., 1800.);
                    let y = g.entre(-400., 1200.);
                    tela.mover_no_corte(gpui::point(px(x), px(y)), window, cx);
                    movimentos += 1;
                }
                valido(tela);
                let antes = gravador.gravado().len();
                tela.soltar_no_corte(cx);
                assert_eq!(
                    gravador.gravado().len(),
                    antes + 1,
                    "uma gravação ao soltar"
                );
                esperadas += 1;
                valido(tela);
            }
            let gasto = inicio.elapsed();
            relatar(
                &format!("{movimentos} movimentos de alça em 60 gestos"),
                gasto,
                Duration::from_secs(10),
            );
            movimentos
        })
        .expect("a janela aberta");
    assert_eq!(gravador.gravado().len(), esperadas);
    let _ = movimentos_de_alca;

    // O transferidor: vinte voltas de ponta a ponta. O ponteiro manda eventos
    // mais depressa do que a tela desenha: quatro movimentos por quadro.
    //
    // ⚠️ O `TestAppContext` desenha a janela suja **ao fim de cada `update`**
    // (o app de verdade desenha no próximo quadro do monitor). Por isso cada
    // `update` aqui leva os quatro movimentos de um quadro, e a medida é do
    // quadro inteiro: os eventos e o desenho.
    let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
    desenhar(&mut visual, 1600., 1000.);
    let mut desenhando = Duration::ZERO;
    let mut quadros = 0u32;
    for volta in 0..8 {
        let (antes, angulo) = janela
            .update(&mut visual, |tela, _window, _cx| {
                let angulo = tela.corte_atual().angle();
                if let Some(edicao) = tela.edicao.as_mut() {
                    edicao.transferidor = Some((600., angulo));
                }
                (gravador.gravado().len(), angulo)
            })
            .expect("a janela aberta");
        for quadro in 0..50 {
            // Vai até +45°, volta até −45°, e para num ângulo qualquer.
            let xs: Vec<f32> = (0..4)
                .map(|i| {
                    let fase = ((quadro * 4 + i) as f32 / 200. * std::f32::consts::TAU).sin();
                    600. + fase * 300. + g.entre(-3., 3.)
                })
                .collect();
            let t = Instant::now();
            janela
                .update(&mut visual, |tela, window, cx| {
                    for x in xs {
                        tela.mover_no_corte(gpui::point(px(x), px(0.)), window, cx);
                    }
                })
                .expect("a janela aberta");
            desenhando += t.elapsed();
            quadros += 1;
        }
        let final_ = 600. + g.entre(-200., 200.);
        janela
            .update(&mut visual, |tela, window, cx| {
                tela.mover_no_corte(gpui::point(px(final_), px(0.)), window, cx);
                valido(tela);
                let angulo_antes = tela.corte_atual().angle();
                tela.soltar_no_corte(cx);
                assert!(
                    gravador.gravado().len() <= antes + 1,
                    "volta {volta}: mais de uma gravação ao soltar"
                );
                if (angulo_antes - angulo).abs() > 1e-3 {
                    assert_eq!(gravador.gravado().len(), antes + 1, "volta {volta}");
                }
            })
            .expect("a janela aberta");
    }
    let por_quadro = desenhando / quadros;
    relatar(
        "um quadro do transferidor (4 movimentos + a foto girada na CPU, média)",
        por_quadro,
        Duration::from_millis(80),
    );
    // Antes do giro por quadro, eram quatro giros por quadro: ~130 ms.
    assert!(por_quadro <= Duration::from_millis(80));

    // O slider do ângulo: mil eventos, uma gravação só, depois da espera.
    let antes = gravador.gravado().len();
    let inicio = Instant::now();
    janela
        .update(&mut visual, |tela, _window, cx| {
            let estado = tela.angulo.clone();
            for i in 0..1_000 {
                let valor = ((i as f32) * 0.37).sin() * ANGULO_MAXIMO;
                estado.update(cx, |_, cx| {
                    cx.emit(SliderEvent::Change(
                        gpui_component::slider::SliderValue::Single(valor),
                    ));
                });
            }
        })
        .expect("a janela aberta");
    // O `update` só entrega os eventos ao fechar, e desenha em seguida: a
    // medida é do lado de fora, e inclui o único giro do quadro.
    let gasto = inicio.elapsed();
    relatar(
        "mil eventos do slider de ângulo num quadro (antes: ~52 s, um giro por evento)",
        gasto,
        Duration::from_millis(500),
    );
    assert!(gasto <= Duration::from_millis(500));
    janela
        .update(&mut visual, |tela, _window, _cx| {
            valido(tela);
            assert!(!tela.exibicao_atrasada, "o quadro refez a foto");
            assert!(
                tela.aberta
                    .as_ref()
                    .and_then(|a| a.desenhada.as_ref())
                    .is_some(),
                "a foto na tela"
            );
        })
        .expect("a janela aberta");
    assert_eq!(
        gravador.gravado().len(),
        antes,
        "no meio do arrasto não grava"
    );
    passar_a_espera(cx);
    assert_eq!(
        gravador.gravado().len(),
        antes + 1,
        "uma gravação para o arrasto inteiro"
    );

    janela
        .update(cx, |tela, window, cx| {
            let mut desfeitos = 0;
            while tela.pode_desfazer() {
                tela.desfazer(window, cx);
                desfeitos += 1;
                valido(tela);
            }
            assert!(desfeitos <= 19, "o histórico passou do teto: {desfeitos}");
            tela.sair_do_corte(cx);
            assert!(!tela.cortando());
        })
        .expect("a janela aberta");
}

/// 🚨 **Centenas de gestos de zoom, trocas de foto e o bruto chegando fora de
/// ordem: nada pinta a foto errada, nada grava, nada escapa.**
#[gpui::test]
fn estresse_zoom_com_bruto_fora_de_ordem(cx: &mut TestAppContext) {
    const FOTOS: usize = 12;
    let (previews, _dir) = previews_descartaveis();
    for i in 0..FOTOS {
        // Cada foto com um tamanho: é por ele que se reconhece o bruto.
        previews
            .save_preview(&format!("id-{i:05}"), &imagem(160 + i as u32, 120))
            .expect("gravar preview");
        previews
            .save_thumbnail(&format!("id-{i:05}"), &imagem(24, 16))
            .expect("gravar miniatura");
    }
    let gravador = Arc::new(GravadorDeMentira::default());
    let janela = janela(cx, previews, gravador.clone());
    janela
        .update(cx, |tela, window, cx| {
            tela.abrir_no_acervo((0..FOTOS).map(foto).collect(), 0, window, cx);
            tela.palco = Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(1200.), px(800.)));
            tela.definir_lado_do_bruto("id-00000", 6000., cx);
        })
        .expect("a janela aberta");

    let mut g = Lcg::novo(61);
    let mut brutos_na_tela = 0usize;
    let mut gestos = 0usize;
    let inicio = Instant::now();
    for passo in 0..3_000 {
        let acao = g.ate(16);
        let atraso = janela
            .update(cx, |tela, window, cx| {
                gestos += 1;
                match acao {
                    0 | 1 => tela.passo_de_zoom(1, cx),
                    2 => tela.passo_de_zoom(-1, cx),
                    3 => tela.alternar_zoom(
                        Some(Ponto {
                            x: g.entre(0., 1200.),
                            y: g.entre(0., 800.),
                        }),
                        cx,
                    ),
                    4 => {
                        tela.z_apertado(cx);
                        tela.z_solto(cx);
                    }
                    5 => tela.zoom_por_tela(if g.ate(2) == 0 { 1 } else { -1 }, cx),
                    6 => {
                        if g.ate(2) == 0 {
                            tela.zoom_ao_inicio(cx)
                        } else {
                            tela.zoom_ao_fim(cx)
                        }
                    }
                    7 => {
                        tela.andar(if g.ate(2) == 0 { 1 } else { -1 }, window, cx);
                        let id = tela.foto_aberta().map(|f| f.id.clone()).expect("aberta");
                        // O lado do bruto nem sempre chega antes do zoom.
                        if g.ate(3) != 0 {
                            tela.definir_lado_do_bruto(&id, g.entre(2000., 9000.), cx);
                        }
                    }
                    8 | 9 => {
                        // Um bruto de qualquer foto, na hora que a rede quiser.
                        let i = g.ate(FOTOS);
                        let id = format!("id-{i:05}");
                        tela.receber_bruto(&id, imagem(2 * (160 + i as u32), 240), cx);
                    }
                    10 => {
                        let i = g.ate(FOTOS);
                        tela.bruto_indisponivel(&format!("id-{i:05}"), cx);
                    }
                    11 => {
                        // Entrar e sair do Enquadrar sem mexer: não grava.
                        tela.alternar_corte(window, cx);
                        tela.alternar_corte(window, cx);
                    }
                    12 => {
                        tela.espaco_apertado(cx);
                        tela.espaco_solto(cx);
                    }
                    13 => tela.soltar_as_teclas(cx),
                    14 => tela.ir_para_nivel(
                        [
                            zoom::Nivel::Encaixar,
                            zoom::Nivel::Preencher,
                            zoom::Nivel::Razao(1.),
                            zoom::Nivel::Razao(16.),
                        ][g.ate(4)],
                        None,
                        cx,
                    ),
                    _ => {}
                }
                // O que o quadro faz a cada desenho.
                tela.acompanhar_a_resolucao(cx);

                // Os invariantes.
                if let Some((cena, vista)) = tela.vista() {
                    assert!(
                        vista.escala.is_finite() && vista.x.is_finite() && vista.y.is_finite(),
                        "passo {passo}: vista com NaN {vista:?} {cena:?}"
                    );
                    assert!((0.0..=1.0).contains(&vista.centro.x), "passo {passo}");
                    assert!((0.0..=1.0).contains(&vista.centro.y), "passo {passo}");
                }
                assert!(!zoom::rotulo_do_nivel(tela.nivel_do_zoom()).is_empty());
                if tela.bruto_na_tela() {
                    brutos_na_tela += 1;
                    let aberta = tela.aberta.as_ref().expect("aberta");
                    let i: u32 = aberta.foto.id[3..].parse().expect("o número");
                    let origem = aberta.origem.as_ref().expect("a origem");
                    assert_eq!(
                        origem.largura,
                        2 * (160 + i),
                        "passo {passo}: o bruto na tela é de outra foto"
                    );
                    assert!(tela.tamanho_da_copia().is_some());
                }
                g.ate(400) as u64
            })
            .expect("a janela aberta");
        cx.executor().advance_clock(Duration::from_millis(atraso));
        cx.run_until_parked();
    }
    relatar(
        &format!("{gestos} gestos de zoom com trocas e brutos"),
        inicio.elapsed(),
        Duration::from_secs(30),
    );
    println!("ℹ️ {brutos_na_tela} passos com o bruto na tela");
    assert!(
        brutos_na_tela > 0,
        "o bruto nunca chegou à tela — o teste não exercitou nada"
    );
    assert!(gravador.gravado().is_empty(), "o zoom gravou alguma coisa");
}
