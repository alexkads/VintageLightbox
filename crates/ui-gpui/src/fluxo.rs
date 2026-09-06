//! O fluxo do estúdio, os onze passos, conferidos de ponta a ponta.
//!
//! # Por que este arquivo existe
//!
//! O dono descreveu o trabalho dele em onze passos e pediu que ele funcione no
//! VintageLightbox como funciona na web. Cada passo tem teste em algum lugar do
//! crate — mas **passo a passo não é fluxo**: os defeitos que doem aqui são os
//! das juntas, e nenhum teste de unidade olha para uma junta.
//!
//! Os três que este arquivo pega e os outros não:
//!
//! 1. **a ordem** — revelar vem antes de classificar, e classificar é o que
//!    autoriza a subir. Uma tela que exigisse nota para revelar passaria em
//!    todos os testes dela e quebraria o fluxo;
//! 2. **o que atravessa** — o id que o site devolve tem de chegar à foto, senão
//!    o balcão do passo 6 não tem em que linha gravar;
//! 3. **o que não pode acontecer** — o passo 10 é uma proibição, e proibição só
//!    se confere olhando o conjunto.
//!
//! ⚠️ **Os passos 8 e 9 são do site**, e o dono disse isso na lista: o cliente
//! baixa e compra pela galeria dele. Aqui eles aparecem como ausência
//! deliberada, e não como esquecimento.

use std::sync::Arc;

use adapters::view_models::PhotoViewModel;
use gpui::TestAppContext;
use image::{DynamicImage, Rgba, RgbaImage};
use infrastructure::cache::preview_manager::PreviewManager;
use tempfile::TempDir;

use crate::app::{Aplicativo, Portas, Tela};
use crate::biblioteca::acervo::mentira::AcervoDeMentira;
use crate::biblioteca::colecoes::mentira::ColecoesDeMentira;
use crate::biblioteca::marcacao::mentira::MarcadorDeMentira;
use crate::entrada::Modo;
use crate::exportacao::porta::mentira::ExportadorDeMentira;
use crate::importacao::explorador::mentira::{
    ExploradorDeMentira, GeradorDeMentira, ImportadorDeMentira, SeletorDeMentira,
};
use crate::impressao::porta::mentira::FolhaDeMentira;
use crate::pos_venda::porta::mentira::PublicadorDeMentira;
use crate::revelacao::persistencia::mentira::GravadorDeMentira;
use crate::revelacao::presets::mentira::GuardaDeMentira;

/// A sessão do operador, já entrada — o passo zero, que a porta do app pede.
fn sessao() -> domain::services::pos_venda::Sessao {
    domain::services::pos_venda::Sessao {
        access_token: "tok".into(),
    }
}

fn foto(nome: &str) -> PhotoViewModel {
    PhotoViewModel {
        id: format!("id-{nome}"),
        name: nome.to_string(),
        path: format!("/fotos/{nome}"),
        ..Default::default()
    }
}

fn previews_com(nomes: &[&str]) -> (Arc<PreviewManager>, TempDir) {
    let dir = TempDir::new().expect("diretório temporário");
    let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
    for nome in nomes {
        let mut imagem = RgbaImage::new(16, 16);
        for pixel in imagem.pixels_mut() {
            *pixel = Rgba([120, 120, 120, 255]);
        }
        previews
            .save_preview(&format!("id-{nome}"), &DynamicImage::ImageRgba8(imagem))
            .expect("gravar preview");
    }
    (previews, dir)
}

/// O estúdio montado: as portas de mentira e a conta do site já entrada.
struct Estudio {
    janela: gpui::WindowHandle<Aplicativo>,
    publicador: Arc<PublicadorDeMentira>,
    marcador: Arc<MarcadorDeMentira>,
    gravador: Arc<GravadorDeMentira>,
    _dir: TempDir,
}

fn abrir_o_estudio(cx: &mut TestAppContext, fotos: Vec<PhotoViewModel>) -> Estudio {
    cx.update(gpui_component::init);
    let nomes: Vec<String> = fotos.iter().map(|f| f.name.clone()).collect();
    let (previews, dir) = previews_com(&nomes.iter().map(String::as_str).collect::<Vec<_>>());

    let publicador = Arc::new(PublicadorDeMentira {
        produtos: vec![domain::services::pos_venda::Produto {
            id: "p1".into(),
            nome: "Foto avulsa".into(),
            preco: "29.90".into(),
            inativo: false,
        }],
        ..Default::default()
    });
    let marcador = Arc::new(MarcadorDeMentira::default());
    let gravador = Arc::new(GravadorDeMentira::default());

    let janela = cx.add_window({
        let previews = previews.clone();
        let publicador = publicador.clone();
        let marcador = marcador.clone();
        let gravador = gravador.clone();
        move |window, cx| {
            Aplicativo::novo(
                fotos,
                previews,
                Vec::new(),
                Portas {
                    gravador,
                    acervo: Arc::new(AcervoDeMentira::default()),
                    exportador: Arc::new(ExportadorDeMentira::default()),
                    publicador,
                    colecoes: Arc::new(ColecoesDeMentira::default()),
                    folha: Arc::new(FolhaDeMentira::default()),
                    marcador,
                    gerador: Arc::new(GeradorDeMentira::default()),
                    guarda_de_presets: Arc::new(GuardaDeMentira::default()),
                    explorador: Arc::new(ExploradorDeMentira::default()),
                    importador: Arc::new(ImportadorDeMentira::default()),
                    seletor: Arc::new(SeletorDeMentira::default()),
                },
                window,
                cx,
            )
        }
    });

    // 🔑 O passo zero: entrar na conta. Desde 6/set/2026 o app abre na porta, e
    // sem responder a ela nenhum dos onze passos acontece.
    janela
        .update(cx, |app, _window, cx| {
            app.escolher_modo(Modo::Online(sessao()), cx);
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    Estudio {
        janela,
        publicador,
        marcador,
        gravador,
        _dir: dir,
    }
}

/// 📸 **Passos 2 e 3, na ordem do dono: revelo primeiro, classifico depois.**
///
/// 🚨 É a interpretação que estava errada antes de 6/set/2026 — a de que só se
/// revela o que já foi classificado. É o contrário: a revelação acontece na foto
/// crua, e a classificação é o que decide o que sobe. Uma tela que exigisse nota
/// para revelar passaria em todos os testes dela e quebraria o fluxo.
#[gpui::test]
fn revelar_vem_antes_de_classificar(cx: &mut TestAppContext) {
    let estudio = abrir_o_estudio(cx, vec![foto("DSC_001.NEF"), foto("DSC_002.NEF")]);

    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));

            // A foto ainda não tem nota nenhuma.
            assert_eq!(app.biblioteca.read(cx).fotos_visiveis()[0].rating, 0);

            // E revelar funciona assim mesmo.
            app.revelar(window, cx);
            assert_eq!(app.tela(), Tela::Revelacao);
            app.revelacao.update(cx, |tela, cx| {
                // Um gesto de slider: o primeiro controle, longe do neutro.
                tela.aplicar_para_teste(0, 1.5, cx);
                tela.gravar_o_que_estiver_pendente();
            });
        })
        .expect("a janela deve estar aberta");

    assert!(
        !estudio.gravador.gravado().is_empty(),
        "a revelação de uma foto sem nota tem de ser gravada"
    );
}

/// 📸 **Passo 3 → 5: classifico, filtro, sinalizo.**
///
/// O que a junta acrescenta aos testes de unidade: a foto classificada **sobe**,
/// o filtro recorta sem perder quem já estava marcado, e a tecla `B` decide o
/// estado com que ela vai ao site.
#[gpui::test]
fn classificar_filtrar_e_sinalizar(cx: &mut TestAppContext) {
    let estudio = abrir_o_estudio(cx, vec![foto("DSC_001.NEF"), foto("DSC_002.NEF")]);

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            // A sessão do cliente, aberta antes: é para onde as fotos vão.
            app.sessoes
                .update(cx, |tela, cx| tela.abrir("g1".into(), cx));

            // Passo 3: classifico só a primeira.
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.na_biblioteca(cx, |tela, cx| tela.dar_nota(4, cx));
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    assert_eq!(
        estudio.publicador.subidas().len(),
        1,
        "classificar é o que autoriza a foto a subir"
    );

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            // Passo 4: filtro as classificadas.
            app.na_biblioteca(cx, |tela, cx| tela.filtrar_por_nota(3, cx));
            assert_eq!(
                app.biblioteca.read(cx).quantas_visiveis(),
                1,
                "a grade mostra só o que passou na nota"
            );

            // Passo 5: sinalizo o que o cliente leva.
            app.na_biblioteca(cx, |tela, cx| tela.selecionar_tudo(cx));
            app.na_biblioteca(cx, |tela, cx| tela.marcar_comprada(cx));
        })
        .expect("a janela deve estar aberta");

    let marcas = estudio.marcador.marcado();
    assert!(
        marcas
            .iter()
            .any(|(_, marca)| matches!(marca, crate::biblioteca::marcacao::Marca::Comprada(true))),
        "a tecla B é o que decide o estado no site: {marcas:#?}"
    );
}

/// 📸 **Passo 6 → 7: o cliente paga, e eu gero o link.**
///
/// 🔑 A junta que este teste pega é a do **id remoto**: sem ele chegar à foto, o
/// balcão não tem em que linha gravar — e o sintoma seria um botão que não faz
/// nada, sem erro nenhum.
#[gpui::test]
fn pagar_no_balcao_e_gerar_o_link(cx: &mut TestAppContext) {
    let mut ja_no_site = foto("DSC_001.NEF");
    ja_no_site.rating = 4;
    ja_no_site.pos_venda_foto_id = Some("remota-1".into());
    let estudio = abrir_o_estudio(cx, vec![ja_no_site, foto("DSC_002.NEF")]);

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.abrir_balcao(cx);
            assert!(app.no_balcao());

            app.balcao.update(cx, |tela, cx| {
                assert_eq!(tela.negociaveis().len(), 1);
                tela.registrar(cx);
            });
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    let negociadas = estudio.publicador.negociadas();
    assert_eq!(negociadas.len(), 1, "o acerto foi para a foto do site");
    assert_eq!(negociadas[0].0, "remota-1");

    // Passo 7: o link do cliente. Ele só existe depois de a galeria existir, e
    // aqui ela nasce da publicação.
    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.publicar(cx);
            app.pos_venda.update(cx, |tela, cx| {
                tela.entrar_para_teste(cx);
                tela.preencher_para_teste("Ensaio da Ana", "ana@x.com", window, cx);
                tela.publicar(cx);
                tela.colher(cx);
                tela.pedir_o_link(cx);
                tela.colher(cx);
                assert!(
                    tela.link().is_some(),
                    "o passo 7 do fluxo: gero o link para o cliente"
                );
            });
        })
        .expect("a janela deve estar aberta");
}

/// 🚨 **Passo 10: nunca apagar a base local. Só o dono.**
///
/// É uma proibição, e por isso o teste olha o conjunto: apagar da grade tira a
/// foto do **catálogo**, e o arquivo continua no disco. É o "Remove from
/// Catalog" do Lightroom, e a confirmação diz isso.
///
/// ⚠️ O único `remove_file` do repositório inteiro é o do modo `Move` da
/// importação, que apaga a **origem** depois de copiar — e a tela avisa antes,
/// porque é o único gesto irreversível do app.
#[gpui::test]
fn apagar_tira_do_catalogo_e_nao_do_disco(cx: &mut TestAppContext) {
    let estudio = abrir_o_estudio(cx, vec![foto("DSC_001.NEF"), foto("DSC_002.NEF")]);

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.na_biblioteca(cx, |tela, cx| tela.pedir_para_apagar(cx));
            assert_eq!(
                app.biblioteca.read(cx).confirmando_apagar(),
                Some(1),
                "apagar pede confirmação — a tecla fica ao lado das treze de triagem"
            );
            app.na_biblioteca(cx, |tela, cx| tela.apagar_confirmado(cx));
        })
        .expect("a janela deve estar aberta");

    assert_eq!(
        estudio.marcador.apagados(),
        vec!["id-DSC_001.NEF".to_string()],
        "sai do catálogo…"
    );
    // …e nada mais: não há porta neste app que apague arquivo, fora o `Move` da
    // importação. A conferência de verdade é `grep remove_file crates/`, e ela
    // devolve **uma** linha.
}

/// 📸 **Passo 11: a Revelação abre a foto que só existe na nuvem.**
///
/// 🚨 **Este teste falha hoje**, e é ele que descreve o que falta: a Revelação
/// lê os pixels do `PreviewManager`, que é disco local. Uma foto que subiu e
/// cujo arquivo local não está aqui — outro computador do estúdio, cache limpo,
/// foto enviada pelo site — abre vazia, sem erro nenhum.
///
/// A web já faz os dois lados (`revelacao/fonte.ts`: *"o módulo de revelação
/// revela as duas — as temporárias e as do acervo"*), e o desktop precisa fazer
/// o mesmo: quando não há preview local e a foto tem id no site, buscar a
/// **cópia de trabalho** por `GET /pos-venda/fotos/{id}/copia-de-trabalho`.
#[gpui::test]
fn a_revelacao_abre_a_foto_que_so_existe_na_nuvem(cx: &mut TestAppContext) {
    // Sem preview local: `abrir_o_estudio` grava preview de todas, então esta
    // entra depois, com um nome que não foi gravado.
    let mut so_na_nuvem = foto("DSC_099.NEF");
    so_na_nuvem.rating = 5;
    so_na_nuvem.pos_venda_foto_id = Some("remota-99".into());

    let estudio = abrir_o_estudio(cx, vec![foto("DSC_001.NEF")]);
    estudio
        .janela
        .update(cx, |app, _window, cx| {
            app.biblioteca.update(cx, |tela, cx| {
                tela.trocar_acervo(vec![so_na_nuvem.clone()], cx)
            });
        })
        .expect("a janela deve estar aberta");

    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.revelar(window, cx);
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    // O laço de espera anda por relógio; aqui a colheita é chamada à mão, como
    // nos outros testes de porta.
    for _ in 0..10 {
        let _ = estudio
            .janela
            .update(cx, |app, _window, cx| app.colher_sincronia(cx));
        cx.run_until_parked();
    }

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            assert!(
                app.revelacao.read(cx).tem_pixels(),
                "a foto que só existe na nuvem tem de abrir na Revelação"
            );
        })
        .expect("a janela deve estar aberta");
}

/// ⚠️ **A foto que o disco já tem não vai à rede.**
///
/// A cópia de trabalho do site custa uma ida à rede por foto. Pedir sempre
/// transformaria a revelação em série — que é como se revela um casamento — em
/// duzentos downloads que o cache local já tinha respondido.
#[gpui::test]
fn a_foto_que_esta_no_disco_nao_vai_a_rede(cx: &mut TestAppContext) {
    let mut tem_as_duas = foto("DSC_001.NEF");
    tem_as_duas.pos_venda_foto_id = Some("remota-1".into());
    let estudio = abrir_o_estudio(cx, vec![tem_as_duas]);

    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.revelar(window, cx);
            assert!(
                app.revelacao.read(cx).tem_pixels(),
                "o preview local respondeu"
            );
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    assert!(
        estudio.publicador.baixadas().is_empty(),
        "estando no disco, nada é pedido ao site"
    );
}
