//! 🧲 **Tirar do acervo**: o caminho de volta inteiro, na ordem que importa.
//!
//! A cláusula C21 do contrato da foto (`recordarfotos-e-commerce/docs/
//! CONTRATO_DA_FOTO.md`) diz o que este módulo prende: *desclassificar devolve
//! tudo ao local, e só então apaga da nuvem*. Era a divergência **D14** —
//! no desktop a tecla `0` numa foto do site era recusada com "use Apagar", e
//! "Apagar" removia da nuvem sem trazer nada para cá.

use domain::services::PreviewType;
use gpui::TestAppContext;

use super::{abrir_o_ensaio, Cenario};

/// A pasta das resgatadas, limpa — os cenários do mesmo processo a dividem, e
/// um arquivo deixado por outro faria o resgate gravar `d (1).jpg`.
fn pasta_limpa() -> std::path::PathBuf {
    let pasta = crate::app::resgate::pasta_das_resgatadas();
    let _ = std::fs::remove_dir_all(&pasta);
    pasta
}

/// 🎬 **O bruto vem para cá, é catalogado com os parâmetros, e só então a
/// nuvem perde a foto.**
///
/// A ordem é o cenário: se `tirar_do_site` acontecesse antes do catálogo, uma
/// falha no meio deixaria a foto sem lugar nenhum — que é o desfecho que
/// custou uma foto na web, duas vezes.
#[gpui::test]
fn tirar_do_acervo_traz_o_bruto_antes_de_a_nuvem_perder_a_foto(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            // O site tem o bruto para devolver — é a condição de tudo.
            site: Box::new(|site| site.definir_bruto(vec![1, 2, 3, 4])),
            ..Cenario::default()
        },
    );

    // A foto que volta é catalogada como qualquer foto do cartão: o acervo de
    // mentira já a conhece pelo caminho que o resgate vai gravar.
    let caminho = pasta_limpa().join("d.jpg");
    let caminho = caminho.to_string_lossy().to_string();
    e.acervo.acrescentar_do_caminho("resgatada-d", &caminho);

    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "0");
    e.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.fotos_na_pergunta_de_tirar_do_acervo(),
            vec!["d.jpg".to_string()],
            "pergunta antes, como a web"
        );
    });
    assert!(
        e.site.tiradas().is_empty(),
        "e nada sai da nuvem enquanto a pergunta está na tela"
    );

    e.detalhe(cx, |tela, _w, cx| tela.confirmar_tirar_do_acervo(cx));
    for _ in 0..40 {
        if !e.site.tiradas().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        e.esperar(cx);
    }

    // 1 · O bruto foi pedido, e é o bruto — não a versão revelada (C3).
    assert_eq!(e.site.originais(), vec!["d".to_string()]);
    assert!(
        std::path::Path::new(&caminho).exists(),
        "o arquivo está no disco desta máquina: {caminho}"
    );

    // 2 · Ele foi catalogado, dentro da sessão aberta.
    let importados = e.importador.importados();
    assert!(
        importados
            .iter()
            .any(|(arquivos, opcoes)| arquivos.contains(&caminho) && opcoes.sessao_id.is_some()),
        "o arquivo entrou no catálogo, na sessão: {importados:?}"
    );

    // 3 · Os parâmetros voltaram com ela — senão a foto volta crua e a nota
    // seguinte a sobe crua.
    assert!(
        e.gravador
            .gravado()
            .iter()
            .any(|(id, _, _)| id == "resgatada-d"),
        "a receita da nuvem foi gravada na foto local: {:?}",
        e.gravador.gravado()
    );

    // 4 · **Só então** a nuvem perdeu a foto.
    assert_eq!(e.site.tiradas(), vec!["d".to_string()]);
    e.app(cx, |app, _w, _cx| {
        let desfecho = app.ultimo_resgate().expect("o lote terminou");
        assert_eq!(desfecho.voltaram, vec!["d.jpg".to_string()]);
        assert!(desfecho.ficaram.is_empty());
    });
}

/// 🎬 **Sem o bruto, nada é apagado** — a foto continua no acervo, com a nota
/// que tinha, e a tela diz qual ficou.
///
/// 🚨 É a metade que protege: a ordem só vale se a falha do primeiro passo
/// impedir o último.
#[gpui::test]
fn sem_o_bruto_a_foto_fica_no_acervo(cx: &mut TestAppContext) {
    // Sem `definir_bruto`, o publicador responde `OriginalIndisponivel`.
    let e = abrir_o_ensaio(cx, Cenario::default());

    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "0");
    e.detalhe(cx, |tela, _w, cx| tela.confirmar_tirar_do_acervo(cx));
    for _ in 0..20 {
        if e.app(cx, |app, _w, _cx| app.ultimo_resgate().is_some()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        e.esperar(cx);
    }

    assert!(
        e.site.tiradas().is_empty(),
        "sem cópia local, a nuvem não perde nada"
    );
    e.app(cx, |app, _w, _cx| {
        let desfecho = app.ultimo_resgate().expect("o lote terminou");
        assert!(desfecho.voltaram.is_empty());
        assert_eq!(desfecho.ficaram, vec!["d.jpg".to_string()]);
        let avisos = app.avisos_para_teste();
        assert!(
            avisos.iter().any(|(t, erro)| *erro && t.contains("d.jpg")),
            "e a tela diz qual ficou: {avisos:?}"
        );
    });
}

/// 🎬 **A comprada e a levada no balcão não perdem a nota** — e não bloqueiam
/// as outras do lote.
#[gpui::test]
fn a_comprada_e_a_levada_ficam_como_estao(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(cx, Cenario::default());

    // `c` é a comprada do cenário.
    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("c", cx));
    e.teclar(cx, "0");
    e.detalhe(cx, |tela, _w, _cx| {
        assert!(
            tela.fotos_na_pergunta_de_tirar_do_acervo().is_empty(),
            "a comprada nem chega à pergunta"
        );
        assert!(
            tela.erro().is_some_and(|f| f.contains("não perde a nota")),
            "e a tela diz por quê: {:?}",
            tela.erro()
        );
    });
    assert!(e.site.tiradas().is_empty());
    assert!(
        e.site.originais().is_empty(),
        "nem o bruto é pedido: não há o que resgatar"
    );
}

/// A prévia local não sobrevive à foto que saiu do acervo.
///
/// 🔑 O id do site deixa de existir; o cache sob ele viraria lixo que ninguém
/// mais atualiza — e a próxima foto a ganhar aquele id veria a receita alheia.
#[gpui::test]
fn a_previa_local_da_foto_que_saiu_nao_fica_para_tras(cx: &mut TestAppContext) {
    let e = abrir_o_ensaio(
        cx,
        Cenario {
            site: Box::new(|site| site.definir_bruto(vec![9, 9, 9])),
            ..Cenario::default()
        },
    );
    let caminho = pasta_limpa().join("d.jpg");
    let caminho = caminho.to_string_lossy().to_string();
    e.acervo.acrescentar_do_caminho("resgatada-d2", &caminho);
    let chave = crate::revelacao::persistencia::chave_da_revelada("site:d");
    e.previews
        .save_thumbnail(
            &chave,
            &image::DynamicImage::ImageRgb8(image::RgbImage::new(4, 4)),
        )
        .expect("a prévia local de antes");

    e.detalhe(cx, |tela, _w, cx| tela.focar_foto("d", cx));
    e.teclar(cx, "0");
    e.detalhe(cx, |tela, _w, cx| tela.confirmar_tirar_do_acervo(cx));
    for _ in 0..40 {
        if !e.site.tiradas().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        e.esperar(cx);
    }

    assert_eq!(e.site.tiradas(), vec!["d".to_string()]);
    assert!(
        !e.previews.tem(&chave, PreviewType::Thumbnail),
        "a prévia local saiu junto com a foto"
    );
}
