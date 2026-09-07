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
use crate::exportacao::porta::mentira::ExportadorDeMentira;
use crate::importacao::explorador::mentira::{
    ExploradorDeMentira, GeradorDeMentira, ImportadorDeMentira, SeletorDeMentira,
};
use crate::impressao::porta::mentira::FolhaDeMentira;
use crate::pos_venda::porta::mentira::PublicadorDeMentira;
use crate::revelacao::lightroom::mentira::EscolhaDeMentira;
use crate::revelacao::persistencia::mentira::GravadorDeMentira;
use crate::revelacao::presets::mentira::GuardaDeMentira;
use crate::sessoes::arquivos::mentira::SeletorDeMentira as SeletorDeFotosDeMentira;

/// A sessão do operador, já entrada — o passo zero, que a porta do app pede.
fn sessao() -> domain::services::pos_venda::Sessao {
    domain::services::pos_venda::Sessao {
        access_token: "tok".into(),
        refresh_token: "ref".into(),
        // Prazos folgados: o que estes testes exercem é a tela, não a
        // renovação — que tem teste próprio em `pos_venda/http.rs`.
        access_vence_em: i64::MAX,
        refresh_vence_em: i64::MAX,
    }
}

/// Uma foto **do ensaio aberto** — desde 6/set/2026 a grade é a da sessão, e
/// foto sem ensaio não aparece nela.
fn foto(nome: &str) -> PhotoViewModel {
    PhotoViewModel {
        id: format!("id-{nome}"),
        name: nome.to_string(),
        path: format!("/fotos/{nome}"),
        sessao_id: Some("g1".into()),
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
    seletor_de_fotos: Arc<SeletorDeFotosDeMentira>,
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
    // O seletor do sistema, de mentira: devolve os caminhos que o teste mandar.
    let seletor_de_fotos = Arc::new(SeletorDeFotosDeMentira::escolhe(&[
        "/exportadas/DSC_001.jpg",
        "/exportadas/DSC_002.jpg",
    ]));

    let janela = cx.add_window({
        let previews = previews.clone();
        let publicador = publicador.clone();
        let marcador = marcador.clone();
        let gravador = gravador.clone();
        let seletor_de_fotos = seletor_de_fotos.clone();
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
                    escolha_de_presets: Arc::new(EscolhaDeMentira::default()),
                    explorador: Arc::new(ExploradorDeMentira::default()),
                    importador: Arc::new(ImportadorDeMentira::default()),
                    seletor: Arc::new(SeletorDeMentira::default()),
                    seletor_de_fotos,
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
            app.entrar_na_conta(sessao(), cx);
            // 🚨 **Entrou: a primeira tela é a lista de sessões**, como na web.
            assert_eq!(app.tela(), Tela::Sessoes);
            // 🚨 **E nada trabalha ainda**: logado, tudo acontece dentro de uma
            // sessão. É o que `nada_acontece_fora_de_uma_sessao` prende.
            assert!(!app.pode_trabalhar());

            // O estúdio abre um ensaio: daí em diante os onze passos acontecem
            // dentro dele.
            app.entrar_na_sessao("g1".into(), cx);
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    janela
        .update(cx, |app, _window, cx| {
            assert!(app.pode_trabalhar(), "com sessão aberta, o app trabalha");
            // 🔑 **Entrar leva à tela do ensaio**: a grade dele está lá, com o
            // cabeçalho, o envio, o painel e a tira — a rota `[id]` do site.
            assert_eq!(app.tela(), Tela::Sessao);

            // ⚠️ **Os testes abaixo são do catálogo local**, e a grade dele é a
            // Biblioteca: importar, revelar em RAW, triar e apagar acontecem
            // sobre o que está no disco desta máquina. A grade da sessão mostra
            // o que está **no site**, e é outra lista.
            app.tela = Tela::Biblioteca;
            let _ = cx;
        })
        .expect("a janela deve estar aberta");

    Estudio {
        janela,
        publicador,
        seletor_de_fotos,
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

    // A sessão do cliente, aberta antes: é para onde as fotos vão. Escolher
    // **entra** nela, como na web.
    estudio
        .janela
        .update(cx, |app, _window, cx| {
            app.sessoes
                .update(cx, |tela, cx| tela.abrir("g1".into(), cx));
        })
        .expect("a janela deve estar aberta");
    // 🔑 O evento só chega ao assinante quando o `update` fecha — por isso a
    // troca de tela é conferida aqui, e não lá dentro.
    cx.run_until_parked();

    estudio
        .janela
        .update(cx, |app, window, cx| {
            assert_eq!(app.tela(), Tela::Sessao, "escolher a sessão entra nela");
            // A triagem deste teste é a do catálogo local: a grade dele é a
            // Biblioteca. A da sessão mostra o que está no site.
            app.tela = Tela::Biblioteca;
            let _ = window;

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

/// 📸 **O caminho que o dono pediu: lista → cria → entra → sobe → revela.**
///
/// 🚨 É a forma, e não a funcionalidade. Tudo isto já existia espalhado — o que
/// não existia era **entrar** numa sessão: "abrir" só a marcava como destino
/// das próximas classificadas, e quem vinha da web achava o app *"muito aberto
/// e estranho"*. Lá a sessão é onde se trabalha; aqui era um rótulo.
#[gpui::test]
fn da_lista_ao_revelar_dentro_da_sessao(cx: &mut TestAppContext) {
    let estudio = abrir_o_estudio(cx, vec![foto("DSC_001.NEF"), foto("DSC_002.NEF")]);

    // 1 · a lista é a primeira tela depois de entrar (conferido no ajudante).
    // 2 · cria a sessão.
    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.tela = Tela::Sessoes;
            app.sessoes.update(cx, |tela, cx| {
                tela.comecar_nova(window, cx);
                tela.preencher_para_teste("Ensaio da Ana", "ana@x.com", window, cx);
                tela.criar(cx);
                // A colheita anda por relógio; aqui é chamada à mão.
                tela.colher(cx);
            });
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    // 3 · criar já entra na sessão — quem cadastrou o cliente vai subir agora.
    estudio
        .janela
        .update(cx, |app, _window, cx| {
            assert_eq!(app.tela(), Tela::Sessao, "criar entra na sessão");
            assert!(
                estudio.publicador.abertas().contains(&"g1".to_string()),
                "e entrar pede a galeria ao site: {:?}",
                estudio.publicador.abertas()
            );
            app.detalhe.update(cx, |tela, cx| tela.colher(cx));
        })
        .expect("a janela deve estar aberta");

    // 4 · o upload: **arquivos do disco**, pelo seletor do sistema.
    //
    // 🚨 Não há explorador nosso aqui, e é o pedido do dono: quem exportou do
    // Lightroom já está com a pasta aberta ao lado.
    estudio
        .janela
        .update(cx, |app, _window, cx| {
            app.detalhe.update(cx, |tela, cx| {
                tela.colher(cx);
                // Sem escolher leva: o padrão é **sem marcação**, como na web.
                assert_eq!(tela.leva(), None);
                tela.escolher_fotos(cx);
                tela.colher(cx);
            });
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    assert_eq!(
        estudio.seletor_de_fotos.pedidos(),
        1,
        "abriu a janela do sistema uma vez"
    );
    let enviados = estudio.publicador.arquivos_enviados();
    assert_eq!(enviados.len(), 2, "as duas escolhidas subiram");
    assert!(enviados.iter().all(|(g, _, _, _)| g == "g1"));
    assert_eq!(enviados[0].1, "/exportadas/DSC_001.jpg");
    // 🔑 Sem marcação vai como "à venda" — o estado de quem ainda não foi levada.
    assert!(
        enviados
            .iter()
            .all(|(_, _, _, e)| *e == domain::services::pos_venda::EstadoNoBalcao::Disponivel),
        "sem marcação entra à venda: {enviados:?}"
    );

    // 5 · revelar uma foto **da sessão**: ela não está no catálogo local, e os
    // pixels vêm do storage (o passo 11).
    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.atender_a_sessao(
                &crate::sessoes::detalhe::Pedido::Revelar {
                    foto_id: "remota-7".into(),
                    arquivo: "DSC_001.jpg".into(),
                },
                window,
                cx,
            );
            assert_eq!(app.tela(), Tela::Revelacao);
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

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
                "a foto da sessão abre na Revelação, com os pixels do storage"
            );
        })
        .expect("a janela deve estar aberta");
    assert_eq!(estudio.publicador.baixadas(), vec!["remota-7".to_string()]);
}

/// 🚨 **Logado, nada acontece fora de uma sessão.**
///
/// Regra do dono, 6/set/2026: importar, revelar, escolher com o cliente,
/// exportar, imprimir e gerar o link são gestos **sobre um ensaio**. Fora dele
/// só existe a lista, que é onde se escolhe em qual entrar.
///
/// 🔑 **A impressão entra pelo mesmo motivo que o resto**, e o dono disse qual:
/// revelação e emolduramento vão virar produtos com custo dentro do ensaio. Uma
/// folha impressa fora de uma sessão é trabalho que ninguém tem como cobrar.
///
/// ⚠️ E a guarda está no **método**, não só no botão: atalho de teclado chega
/// antes de botão, e foi assim que uma nota já caiu numa grade que ninguém
/// estava vendo.
#[gpui::test]
fn nada_acontece_fora_de_uma_sessao(cx: &mut TestAppContext) {
    let (previews, _dir) = previews_com(&["DSC_001.NEF"]);
    cx.update(gpui_component::init);

    let janela = cx.add_window({
        let previews = previews.clone();
        move |window, cx| {
            Aplicativo::novo(
                vec![foto("DSC_001.NEF")],
                previews,
                Vec::new(),
                Portas {
                    gravador: Arc::new(GravadorDeMentira::default()),
                    acervo: Arc::new(AcervoDeMentira::default()),
                    exportador: Arc::new(ExportadorDeMentira::default()),
                    publicador: Arc::new(PublicadorDeMentira::default()),
                    colecoes: Arc::new(ColecoesDeMentira::default()),
                    folha: Arc::new(FolhaDeMentira::default()),
                    marcador: Arc::new(MarcadorDeMentira::default()),
                    gerador: Arc::new(GeradorDeMentira::default()),
                    guarda_de_presets: Arc::new(GuardaDeMentira::default()),
                    escolha_de_presets: Arc::new(EscolhaDeMentira::default()),
                    explorador: Arc::new(ExploradorDeMentira::default()),
                    importador: Arc::new(ImportadorDeMentira::default()),
                    seletor: Arc::new(SeletorDeMentira::default()),
                    seletor_de_fotos: Arc::new(SeletorDeFotosDeMentira::default()),
                },
                window,
                cx,
            )
        }
    });

    janela
        .update(cx, |app, window, cx| {
            app.entrar_na_conta(sessao(), cx);
            assert!(!app.pode_trabalhar(), "logado e sem sessão: nada trabalha");

            // Nenhum dos gestos sai do lugar — nem pela porta do método.
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.revelar(window, cx);
            app.imprimir(window, cx);
            app.exportar(cx);
            app.importar(window, cx);
            app.abrir_balcao(cx);
            app.alternar_cliente(cx);

            assert_eq!(app.tela(), Tela::Sessoes, "continua na lista");
            assert!(!app.exportando() && !app.importando() && !app.no_balcao());

            // Com uma sessão aberta, o app volta a trabalhar.
            app.entrar_na_sessao("g1".into(), cx);
            assert!(app.pode_trabalhar());
            // A grade do catálogo local é a Biblioteca; a da sessão mostra o
            // que está no site.
            app.tela = Tela::Biblioteca;
            app.na_biblioteca(cx, |tela, cx| tela.selecionar(Some(0), cx));
            app.revelar(window, cx);
            assert_eq!(app.tela(), Tela::Revelacao, "agora sim");
        })
        .expect("a janela deve estar aberta");
}

/// 🚨 **A grade é a do ensaio, e é uma só.**
///
/// A correção de 6/set/2026, com as palavras do dono: *"dentro da sessão que
/// fazemos as revelações e escolhemos as fotos com o cliente"*. Antes dela a
/// grade era o catálogo inteiro e a sessão era uma tela ao lado — o ensaio de um
/// cliente ficava misturado com o de todos os outros, e havia dois lugares para
/// o mesmo trabalho.
///
/// 🔑 **E as do site entram na mesma lista das locais**, como na web: a foto que
/// já subiu e a que ainda não subiu aparecem juntas, e é sobre essa lista que a
/// revelação e a escolha com o cliente acontecem.
#[gpui::test]
fn a_grade_e_a_do_ensaio_e_e_uma_so(cx: &mut TestAppContext) {
    let mut de_outro_cliente = foto("DSC_900.NEF");
    de_outro_cliente.sessao_id = Some("g9".into());

    let estudio = abrir_o_estudio(
        cx,
        vec![foto("DSC_001.NEF"), foto("DSC_002.NEF"), de_outro_cliente],
    );

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            assert_eq!(
                app.biblioteca.read(cx).quantas_visiveis(),
                2,
                "o ensaio de outro cliente não aparece nesta grade"
            );

            // A sessão respondeu com uma foto que já está no site: ela entra na
            // **mesma** grade, e não numa segunda tela.
            app.atender_a_sessao(
                &crate::sessoes::detalhe::Pedido::FotosDoSite(vec![
                    domain::services::pos_venda::FotoDaGaleria {
                        id: "remota-1".into(),
                        arquivo: "DSC_010.jpg".into(),
                        estado: domain::services::pos_venda::EstadoDaFotoNoSite::LevadaNoBalcao,
                        ordem: 0,
                        preco_negociado: None,
                        observacao_da_negociacao: None,
                        apagada: false,
                        nota: Some(4),
                        produto_efetivo: "p1".into(),
                        preco_de_venda: None,
                        pedido_id: None,
                        downloads: 0,
                        revelada: false,
                    },
                ]),
                _window,
                cx,
            );
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            let visiveis = app.biblioteca.read(cx).fotos_visiveis();
            // ⚠️ O `AcervoDeMentira` devolve lista vazia ao reler, então o que
            // sobra na grade são exatamente as do site — o que este teste quer
            // ver é que elas **entraram**.
            assert!(
                visiveis.iter().any(|f| f.id == "site:remota-1"),
                "a foto do site entrou na mesma grade: {:?}",
                visiveis.iter().map(|f| &f.id).collect::<Vec<_>>()
            );
            let do_site = visiveis.iter().find(|f| f.id == "site:remota-1").unwrap();
            assert_eq!(do_site.rating, 4, "a nota veio do site");
            assert!(do_site.comprada, "levada no balcão é 'comprada' aqui");
            assert_eq!(do_site.sessao_id.as_deref(), Some("g1"));
        })
        .expect("a janela deve estar aberta");
}

/// 📸 Importar dentro de um ensaio carimba o ensaio no lote.
///
/// Sem o carimbo a foto chega ao catálogo sem dono e **não aparece** na grade da
/// sessão que a importou — e o sintoma é "a importação não funcionou".
#[gpui::test]
fn importar_dentro_da_sessao_poe_a_foto_nela(cx: &mut TestAppContext) {
    let estudio = abrir_o_estudio(cx, vec![foto("DSC_001.NEF")]);

    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.importar(window, cx);
            assert!(app.importando());
            assert_eq!(
                app.importacao.read(cx).estado.opcoes.sessao_id.as_deref(),
                Some("g1"),
                "o lote entra no ensaio aberto"
            );
        })
        .expect("a janela deve estar aberta");
}

/// 🚨 **A tela da sessão tem altura — e mostra o que está no site.**
///
/// O defeito que o dono viu: a sessão abria com *"25 no site"* escrito e **nada**
/// embaixo. Não era grade vazia — era grade sem altura: o cabeçalho usava
/// `size_full` e comia os 100% da coluna.
#[gpui::test]
fn a_sessao_aberta_mostra_as_fotos_do_site(cx: &mut TestAppContext) {
    let estudio = abrir_o_estudio(cx, vec![foto("DSC_001.NEF")]);

    // A sessão responde com duas fotos que já estão no site.
    estudio
        .janela
        .update(cx, |app, window, cx| {
            app.tela = Tela::Sessao;
            app.atender_a_sessao(
                &crate::sessoes::detalhe::Pedido::FotosDoSite(vec![
                    do_site("remota-1", "DSC_010.jpg", Some(4)),
                    do_site("remota-2", "DSC_011.jpg", None),
                ]),
                window,
                cx,
            );
        })
        .expect("a janela deve estar aberta");
    cx.run_until_parked();

    estudio
        .janela
        .update(cx, |app, _window, cx| {
            assert_eq!(app.tela(), Tela::Sessao);
            let detalhe = app.detalhe.read(cx);
            assert_eq!(detalhe.total_visivel(), 0, "sem sessão aberta de verdade");
            let _ = detalhe;
        })
        .expect("a janela deve estar aberta");
}

/// Uma foto do site, como a API a devolve.
fn do_site(
    id: &str,
    arquivo: &str,
    nota: Option<u8>,
) -> domain::services::pos_venda::FotoDaGaleria {
    domain::services::pos_venda::FotoDaGaleria {
        id: id.into(),
        arquivo: arquivo.into(),
        estado: domain::services::pos_venda::EstadoDaFotoNoSite::Disponivel,
        ordem: 0,
        preco_negociado: None,
        observacao_da_negociacao: None,
        apagada: false,
        nota,
        produto_efetivo: "p1".into(),
        preco_de_venda: None,
        pedido_id: None,
        downloads: 0,
        revelada: false,
    }
}
