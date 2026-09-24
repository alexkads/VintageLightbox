//! 🎬 Fluxos da janela GPUI até as portas do app, com serviços de memória.
//!
//! # O que estes cenários são
//!
//! A janela é **simulada pelo `TestAppContext`**, com o `Root` do `gpui-component` e o
//! [`Aplicativo`] dentro, as mesmas teclas ligadas (`app::init`,
//! `importacao::tela::init`, `cliente::init`) e o tema do site aplicado — e
//! cada passo passa pela porta que o operador usa: a tecla de verdade
//! (`simulate_keystrokes`) quando o gesto tem tecla, o pedido que o botão emite
//! quando não tem. Depois de cada passo o cenário afirma **três coisas**: a
//! tela que está na frente, o estado que ela mostra e o que foi pedido às
//! portas.
//!
//! Estes cenários exercitam o `render`, os cliques e o despacho do GPUI na janela
//! de teste. **Não mostram a janela nativa nem falam com a API local**. Para conferir o
//! binário e a janela reais, use `VLB_ROTEIRO` + `VLB_FOTOS` em build debug;
//! esse é um teste visual separado, não uma evidência produzida por `cargo test`.
//!
//! 🔑 **O `render` do app inteiro roda a cada passo.** Com `test-support`, o
//! GPUI redesenha a janela suja ao fim de todo `update` (`flush_effects`) — um
//! pânico ao desenhar qualquer tela derruba o cenário, e é justamente o tipo de
//! defeito que um teste de método não pega.
//!
//! # 🚨 O que eles nunca fazem
//!
//! **Nada sai da máquina.** O site é o [`PublicadorDeMentira`], o catálogo é o
//! [`GravadorDeMentira`] e o [`AcervoDeMentira`], o disco é o
//! [`ImportadorDeMentira`] e o [`ExportadorDeMentira`]. A API de produção tem
//! clientes e dinheiro reais: um cenário que falasse com ela venderia foto de
//! mentira a um cliente de verdade.
//!
//! E nada escreve nas pastas de quem roda a suíte: o tema, a escolha do
//! "Sincronizar", o JPEG baixado, a lembrança do caixa e a do estúdio vão para
//! pastas temporárias em `cfg(test)`.
//!
//! ⚠️ **Por que estes cenários não usam o binário contra a pilha local.** Eles
//! foram escritos para testar as regras da interface rapidamente e sem dados
//! externos. O roteiro de depuração do binário cobre a janela real contra
//! `make up`, com fotos da tela, mas precisa de suas próprias verificações para
//! ser considerado um E2E automatizado completo.
//!
//! | Módulo | O pedaço do fluxo |
//! |---|---|
//! | [`conta`] | a porta, `/auth/me`, o tema, o menu lateral, o menu da conta e o Sair |
//! | [`sessoes`] | a lista, a busca, os recortes, a sessão nova, a retenção e o caixa |
//! | [`galeria`] | dentro da sessão: importar, classificar, levar, negociar, imprimir, exportar, o cliente e o link |
//! | [`atendimento`] | **as fotos**: quais entram em cada recorte, o que cada gesto faz **nelas**, a tira da revelação e o que o cliente vê |
//! | [`caixa`] | o caixa flutuante na galeria e na revelação |
//! | [`cliente`] | a segunda tela acompanhando a galeria e a revelação |
//! | [`revelacao`] | a tira, os sliders, o histórico, as abas, a curva e as predefinições |
//! | [`enquadrar`] | girar, espelhar, endireitar, proporção e alças |
//! | [`zoom`] | as teclas do zoom, a folha de atalhos e o bruto em resolução cheia |
//! | [`lote`] | sincronizar, zerar, a comprada, "Baixar JPEG" e "Salvar na galeria" |
//! | [`segundo_plano`] | minimizar, fechar com envio pendente e sair quando a fila esvazia |

mod atendimento;
mod caixa;
mod cliente;
mod conta;
mod enquadrar;
mod galeria;
mod lote;
mod nova_sessao;
mod revelacao;
mod segundo_plano;
mod sessoes;
mod zoom;

use std::sync::Arc;
use std::time::Duration;

use adapters::view_models::PhotoViewModel;
use domain::entities::Preset;
use domain::services::pos_venda::{
    EstadoDaFotoNoSite, Estudio as EstudioDoSite, FotoDaGaleria, GaleriaDoPainel, Produto, Sessao,
};
use gpui::{
    AppContext as _, Context, Entity, TestAppContext, VisualTestContext, Window, WindowHandle,
};
use gpui_component::Root;
use image::{DynamicImage, Rgba, RgbaImage};
use infrastructure::cache::preview_manager::PreviewManager;
use serde_json::{json, Value};
use tempfile::TempDir;

use crate::app::{Aplicativo, Portas, Tela};
use crate::atualizacao::porta::mentira::AtualizadorDeMentira;
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
use crate::revelacao::lightroom::Arquivo;
use crate::revelacao::persistencia::{self, mentira::GravadorDeMentira};
use crate::revelacao::presets::mentira::GuardaDeMentira;
use crate::revelacao::reposicao::mentira::RepositorDeMentira;
use crate::revelacao::tela::Revelacao;
use crate::sessoes::arquivos::mentira::SeletorDeMentira as SeletorDeFotosDeMentira;
use crate::sessoes::detalhe::Detalhe;

/// O ensaio que todos os cenários abrem.
pub(super) const GALERIA: &str = "g1";

/// As fotos locais do ensaio: importadas nesta máquina, ainda sem nota.
pub(super) const LOCAIS: [&str; 2] = ["DSC_101.jpg", "DSC_102.jpg"];

/// A conta que a porta devolve.
pub(super) fn sessao() -> Sessao {
    Sessao {
        access_token: "tok".into(),
        refresh_token: "ref".into(),
        access_vence_em: i64::MAX,
        refresh_vence_em: i64::MAX,
    }
}

/// Uma imagem de um tom só, `largura`×`altura`.
pub(super) fn imagem(largura: u32, altura: u32, tom: u8) -> DynamicImage {
    let mut img = RgbaImage::new(largura, altura);
    for pixel in img.pixels_mut() {
        *pixel = Rgba([tom, tom, tom, 255]);
    }
    DynamicImage::ImageRgba8(img)
}

/// Uma foto do catálogo local, do ensaio aberto.
pub(super) fn local(nome: &str) -> PhotoViewModel {
    PhotoViewModel {
        id: format!("id-{nome}"),
        name: nome.to_string(),
        path: format!("/ensaios/{nome}"),
        sessao_id: Some(GALERIA.into()),
        ..Default::default()
    }
}

/// Uma foto do site, como `GET /pos-venda/galerias/{id}` a devolve.
pub(super) fn do_site(
    id: &str,
    ordem: i32,
    estado: EstadoDaFotoNoSite,
    nota: Option<u8>,
) -> FotoDaGaleria {
    FotoDaGaleria {
        id: id.into(),
        arquivo: format!("{id}.jpg"),
        estado,
        ordem,
        preco_negociado: None,
        observacao_da_negociacao: None,
        apagada: false,
        nota,
        produto_efetivo: "p1".into(),
        preco_de_venda: None,
        pedido_id: None,
        downloads: 0,
        revelada: false,
        ajustes: None,
        ..Default::default()
    }
}

/// As fotos do ensaio no site: duas levadas no balcão, uma à venda e uma
/// comprada — a comprada é a que a revelação não pode tocar.
pub(super) fn fotos_do_site() -> Vec<FotoDaGaleria> {
    vec![
        do_site("a", 0, EstadoDaFotoNoSite::LevadaNoBalcao, Some(4)),
        do_site("b", 1, EstadoDaFotoNoSite::LevadaNoBalcao, Some(5)),
        do_site("d", 2, EstadoDaFotoNoSite::Disponivel, Some(3)),
        do_site("c", 3, EstadoDaFotoNoSite::Comprada, Some(5)),
    ]
}

pub(super) fn galeria_do_painel(id: &str, titulo: &str, email: Option<&str>) -> GaleriaDoPainel {
    GaleriaDoPainel {
        id: id.into(),
        titulo: titulo.into(),
        email: email.map(str::to_string),
        whatsapp: None,
        produto_id: "p1".into(),
        user_id: None,
        criada_em_iso: "2026-09-16".into(),
        expira_em: None,
        fotos: Default::default(),
        totais: None,
        ..Default::default()
    }
}

fn foto_json(id: &str, ordem: i64, estado: &str) -> Value {
    json!({
        "id": id, "arquivo": format!("{id}.jpg"), "estado": estado, "ordem": ordem,
        "nota": 4, "apagada_em": null, "produto_efetivo": "p1", "produto_id": null,
        "preco_negociado": null, "observacao_da_negociacao": null
    })
}

/// A galeria como o caixa a lê (`GET /pos-venda/galerias/{id}`).
fn galeria_json() -> Value {
    json!({
        "galeria": { "id": GALERIA, "titulo": "Ensaio da Ana", "estudio_id": "e1" },
        "fotos": [
            foto_json("a", 0, "levada_no_balcao"),
            foto_json("b", 1, "levada_no_balcao"),
            foto_json("d", 2, "disponivel"),
            foto_json("c", 3, "comprada"),
        ],
        "produto": { "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" },
        "produtos": [{ "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" }],
        "avisos": []
    })
}

/// A venda que `POST /pos-venda/caixa/vendas` devolve.
pub(super) fn venda_json(numero: i64, fotos: &[&str], total: i64) -> Value {
    json!({
        "id": format!("v{numero}"), "numero": numero, "caixa_id": "cx", "galeria_id": GALERIA,
        "itens": fotos.iter().map(|f| json!({
            "foto_id": f, "ordem": 0, "arquivo": format!("{f}.jpg"), "faixa": "Digital",
            "cheio_centavos": 4000, "cobrado_centavos": 4000, "desconto_centavos": 0,
            "tipo_de_negociacao": null, "etiqueta": null, "estornada": false,
            "valor_sugerido_de_estorno_centavos": 4000
        })).collect::<Vec<_>>(),
        "pagamentos": [{ "forma": "pix", "valor_centavos": total, "detalhe": null }],
        "total_centavos": total, "troco_centavos": 0,
        "fotografo_id": "f1", "fotografo": "Ana", "atendente_id": "f1", "atendente": "Ana",
        "auxiliar_id": "f1", "auxiliar": "Ana",
        "criada_em": "2026-09-17T10:00:00Z",
        "estornado_centavos": 0, "estornavel_centavos": total,
        "fotos_vendidas": fotos, "estornos": []
    })
}

/// O site de mentira, com o ensaio, os produtos, os estúdios e as respostas
/// JSON do caixa, da conta e da retenção.
fn site_de_mentira(ajustar: impl FnOnce(&mut PublicadorDeMentira)) -> Arc<PublicadorDeMentira> {
    let mut site = PublicadorDeMentira {
        produtos: vec![Produto {
            id: "p1".into(),
            nome: "Digital".into(),
            preco: "30.00".into(),
            inativo: false,
        }],
        estudios: vec![EstudioDoSite {
            id: "e1".into(),
            nome: "Centro".into(),
            cidade: "Gramado".into(),
            foto: Some("https://r2/studios/centro.jpg".into()),
        }],
        galerias: std::sync::Mutex::new(vec![
            galeria_do_painel(GALERIA, "Ensaio da Ana", Some("ana@exemplo.com")),
            galeria_do_painel("g2", "Casamento do João", None),
        ]),
        fotos_da_sessao: std::sync::Mutex::new(fotos_do_site()),
        bruto: std::sync::Mutex::new(Some(PublicadorDeMentira::jpeg(640))),
        ..Default::default()
    };
    ajustar(&mut site);
    let site = Arc::new(site);
    site.responder_json(
        "conta",
        Ok(json!({ "user": { "email": "alex@exemplo.com", "nome": "Alex" } })),
    );
    site.responder_json(
        "retencao",
        Ok(json!({
            "dias_a_venda": 30, "dias_liberadas": 60, "dias_de_aviso": 7,
            "prorrogacao_sem_leitura_dias": 15,
            "apagar_liberada_sem_download": false, "apagar_automaticamente": true,
            "atualizada_em": "2026-09-10T12:00:00Z"
        })),
    );
    site.responder_json(
        "retencao-salva",
        Ok(json!({
            "dias_a_venda": 45, "dias_liberadas": 60, "dias_de_aviso": 7,
            "prorrogacao_sem_leitura_dias": 15,
            "apagar_liberada_sem_download": false, "apagar_automaticamente": true,
            "atualizada_em": "2026-09-17T12:00:00Z"
        })),
    );
    site.responder_json(
        "estudios",
        Ok(json!([{ "id": "e1", "name": "Centro", "city": "Gramado", "is_active": true }])),
    );
    site.responder_json(
        "galerias",
        Ok(
            json!([{ "id": GALERIA, "titulo": "Ensaio da Ana", "email": "ana@exemplo.com",
                    "whatsapp": null, "estudio_id": "e1",
                    "criada_em": "2026-09-16T12:00:00Z",
                    "fotos": { "levadas_no_balcao": 2 } }]),
        ),
    );
    site.responder_json(
        "funcionarios",
        Ok(json!([{ "id": "f1", "nome": "Ana", "ativo": true }])),
    );
    site.responder_json(
        "catalogo",
        Ok(json!([
            { "product": { "id": "p1", "name": "Digital", "price": "30.00", "normal_price": "40.00" } }
        ])),
    );
    site.responder_json("galeria", Ok(galeria_json()));
    site.responder_json("galeria-viva", Ok(galeria_json()));
    site.responder_json("vendas", Ok(json!([])));
    site.responder_json(
        "caixa",
        Ok(json!({
            "id": "cx", "estudio_id": "e1", "operador_email": "op@x",
            "fundo_de_troco_centavos": 0, "aberto_em": "2026-09-17T08:00:00Z",
            "movimentos": [], "vendas": [], "estornos": []
        })),
    );
    site.responder_json("lote", Ok(json!({})));
    site.responder_json("venda", Ok(venda_json(12, &["a", "b"], 8000)));
    site
}

/// O que muda de um cenário para outro.
pub(super) struct Cenario {
    /// As predefinições que o app carregou ao abrir.
    pub presets: Vec<Preset>,
    /// O que o seletor de `.lrtemplate`/`.dtstyle` devolve.
    pub arquivos_de_predefinicao: Vec<Arquivo>,
    /// Mexe no site antes de ele atender.
    pub site: Box<dyn FnOnce(&mut PublicadorDeMentira)>,
    /// Liga o segundo plano (a bandeja) na janela, como o `main.rs`.
    pub segundo_plano: bool,
    /// A cópia do disco anda só quando o cenário manda — o cartão lento.
    pub importador_demorado: bool,
}

impl Default for Cenario {
    fn default() -> Self {
        Self {
            presets: presets_do_sistema(),
            arquivos_de_predefinicao: Vec::new(),
            site: Box::new(|_| {}),
            segundo_plano: false,
            importador_demorado: false,
        }
    }
}

/// Três predefinições do sistema, para prever, aplicar e reordenar.
pub(super) fn presets_do_sistema() -> Vec<Preset> {
    use domain::entities::preset::PresetAdjustments;
    let com = |nome: &str, exposicao: f32| {
        Preset::system(nome, PresetAdjustments::vazia().com("exposure", exposicao))
    };
    vec![com("Claro", 1.0), com("Escuro", -1.0), com("Neutro+", 0.3)]
}

/// O estúdio montado: a janela, as portas de mentira e o cache.
pub(super) struct Estudio {
    pub raiz: WindowHandle<Root>,
    pub app: Entity<Aplicativo>,
    pub site: Arc<PublicadorDeMentira>,
    pub gravador: Arc<GravadorDeMentira>,
    pub acervo: Arc<AcervoDeMentira>,
    pub importador: Arc<ImportadorDeMentira>,
    pub seletor_de_fotos: Arc<SeletorDeFotosDeMentira>,
    pub exportador: Arc<ExportadorDeMentira>,
    pub folha: Arc<FolhaDeMentira>,
    pub guarda: Arc<GuardaDeMentira>,
    /// O cache de prévias do app — é nele que a prévia **revelada local** mora,
    /// e é por ele que os cenários afirmam que a grade vai mostrar o efeito.
    pub previews: Arc<PreviewManager>,
    _dir: TempDir,
}

/// Abre o app **na porta**, sem conta — o passo zero de todo cenário.
pub(super) fn abrir_o_app(cx: &mut TestAppContext, cenario: Cenario) -> Estudio {
    cx.update(|cx| {
        gpui_component::init(cx);
        crate::tema::aplicar(crate::tema::Escolha::Claro, None, cx);
        crate::app::init(cx);
        crate::importacao::tela::init(cx);
        crate::cliente::init(cx);
    });

    let dir = TempDir::new().expect("diretório temporário");
    let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
    for nome in LOCAIS {
        previews
            .save_preview(&format!("id-{nome}"), &imagem(160, 120, 110))
            .expect("gravar o preview local");
    }
    // A cópia de trabalho de duas fotos do site já está no cache (abertas
    // ontem); a `d` não, e é ela que vai buscar no storage.
    for id in ["a", "b", "c"] {
        previews
            .save_preview(
                &persistencia::chave_do_trabalho(&format!("site:{id}")),
                &imagem(160, 120, 140),
            )
            .expect("gravar a cópia de trabalho");
    }

    let site = site_de_mentira(cenario.site);
    let locais: Vec<PhotoViewModel> = LOCAIS.iter().map(|n| local(n)).collect();
    let acervo = Arc::new(AcervoDeMentira {
        fotos: std::sync::Mutex::new(locais.clone()),
        ..Default::default()
    });
    let gravador = Arc::new(GravadorDeMentira::default());
    let importador = Arc::new(if cenario.importador_demorado {
        ImportadorDeMentira::demorado()
    } else {
        ImportadorDeMentira::default()
    });
    let seletor_de_fotos = Arc::new(SeletorDeFotosDeMentira::escolhe(&[
        "/cartao/DSC_201.jpg",
        "/cartao/DSC_202.jpg",
    ]));
    let exportador = Arc::new(ExportadorDeMentira::default());
    let folha = Arc::new(FolhaDeMentira::default());
    let guarda = Arc::new(GuardaDeMentira::default());
    let escolha = Arc::new(EscolhaDeMentira::com(cenario.arquivos_de_predefinicao));

    let portas = Portas {
        gravador: gravador.clone(),
        acervo: acervo.clone(),
        exportador: exportador.clone(),
        publicador: site.clone(),
        colecoes: Arc::new(ColecoesDeMentira::default()),
        folha: folha.clone(),
        marcador: Arc::new(MarcadorDeMentira::gravando_em(acervo.clone())),
        gerador: Arc::new(GeradorDeMentira::default()),
        repositor: Arc::new(RepositorDeMentira::default()),
        guarda_de_presets: guarda.clone(),
        escolha_de_presets: escolha,
        explorador: Arc::new(ExploradorDeMentira::default()),
        importador: importador.clone(),
        seletor: Arc::new(SeletorDeMentira::default()),
        seletor_de_fotos: seletor_de_fotos.clone(),
        atualizador: Arc::new(AtualizadorDeMentira::default()),
        acervo_de_arquivos: Arc::new(
            crate::backup::porta::mentira::AcervoDeArquivosDeMentira::default(),
        ),
        escolha_do_backup: Arc::new(crate::backup::escolha::mentira::EscolhaDeMentira::default()),
    };

    let mut guardado = None;
    let presets = cenario.presets;
    let segundo_plano = cenario.segundo_plano;
    let raiz = cx.add_window({
        let previews = previews.clone();
        let guardado = &mut guardado;
        move |window, cx| {
            let app = cx.new(|cx| Aplicativo::novo(locais, previews, presets, portas, window, cx));
            if segundo_plano {
                crate::segundo_plano::ligar(app.downgrade(), window, cx);
            }
            *guardado = Some(app.clone());
            Root::new(app, window, cx)
        }
    });
    // A janela do app está na frente, como ao abrir: sem isto a Revelação
    // larga as teclas seguradas a cada quadro (`soltar_as_teclas`).
    raiz.update(cx, |_raiz, window, _cx| window.activate_window())
        .expect("a janela principal está aberta");
    cx.run_until_parked();
    let app = guardado.expect("o aplicativo foi construído");
    let estudio = Estudio {
        raiz,
        app,
        site,
        gravador,
        acervo,
        importador,
        seletor_de_fotos,
        exportador,
        folha,
        guarda,
        previews,
        _dir: dir,
    };
    estudio.esperar(cx);
    estudio
}

/// Abre o app, entra na conta e entra no ensaio — onde quase todo cenário
/// começa. Afirma cada degrau pelo caminho.
pub(super) fn abrir_o_ensaio(cx: &mut TestAppContext, cenario: Cenario) -> Estudio {
    let e = abrir_o_app(cx, cenario);
    e.entrar_na_conta(cx);
    e.app(cx, |app, _w, cx| {
        app.sessoes
            .update(cx, |tela, cx| tela.abrir(GALERIA.into(), cx));
    });
    e.esperar(cx);
    e.app(cx, |app, _w, cx| {
        assert_eq!(app.tela(), Tela::Sessao, "escolher a sessão entra nela");
        assert!(app.pode_trabalhar());
        let detalhe = app.detalhe.read(cx);
        assert_eq!(detalhe.galeria_id(), Some(GALERIA));
        assert_eq!(
            detalhe
                .aberta()
                .map(|a| a.galeria.titulo.clone())
                .as_deref(),
            Some("Ensaio da Ana"),
            "a galeria chegou do site"
        );
    });
    e
}

impl Estudio {
    /// Um gesto no app, com a janela na mão.
    pub fn app<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Aplicativo, &mut Window, &mut Context<Aplicativo>) -> R,
    ) -> R {
        let app = self.app.clone();
        // 🔑 `update_window`, e não `raiz.update`: o clique de verdade chega
        // com o `Root` livre, e é ele que abre os diálogos do `gpui-component`.
        let r = cx
            .update_window(self.raiz.into(), |_raiz, window, cx| {
                app.update(cx, |app, cx| f(app, window, cx))
            })
            .expect("a janela principal está aberta");
        cx.run_until_parked();
        r
    }

    /// Um gesto na Revelação.
    pub fn revelacao<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Revelacao, &mut Window, &mut Context<Revelacao>) -> R,
    ) -> R {
        self.app(cx, |app, window, cx| {
            let tela = app.revelacao.clone();
            tela.update(cx, |tela, cx| f(tela, window, cx))
        })
    }

    /// Um gesto na tela da sessão.
    pub fn detalhe<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Detalhe, &mut Window, &mut Context<Detalhe>) -> R,
    ) -> R {
        self.app(cx, |app, window, cx| {
            let tela = app.detalhe.clone();
            tela.update(cx, |tela, cx| f(tela, window, cx))
        })
    }

    /// Teclas de verdade, na janela principal.
    pub fn teclar(&self, cx: &mut TestAppContext, teclas: &str) {
        let mut visual = VisualTestContext::from_window(self.raiz.into(), cx);
        visual.simulate_keystrokes(teclas);
        cx.run_until_parked();
    }

    /// Solta uma tecla — o `simulate_keystrokes` só aperta, e o `Z`, o
    /// Espaço e o `\` da Revelação fazem coisas diferentes ao soltar.
    pub fn soltar(&self, cx: &mut TestAppContext, tecla: &str) {
        let mut visual = VisualTestContext::from_window(self.raiz.into(), cx);
        visual.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(tecla).expect("uma tecla válida"),
        });
        cx.run_until_parked();
    }

    /// Deixa o relógio andar: as colheitas das telas acordam a cada 100 ms, e a
    /// espera da gravação é de 500 ms.
    pub fn esperar(&self, cx: &mut TestAppContext) {
        for _ in 0..12 {
            cx.executor().advance_clock(Duration::from_millis(150));
            cx.run_until_parked();
        }
    }

    /// O passo zero: a porta responde com a conta.
    pub fn entrar_na_conta(&self, cx: &mut TestAppContext) {
        self.app(cx, |app, _w, cx| {
            assert!(!app.entrou(), "o app abre na porta");
            app.entrar_na_conta(sessao(), cx);
        });
        self.esperar(cx);
    }

    /// Entra na Revelação pelo botão da barra da sessão — a sessão inteira
    /// na tira — e anda até a foto `inicial`.
    pub fn revelar_pela_barra(&self, cx: &mut TestAppContext) {
        self.detalhe(cx, |tela, _w, cx| tela.revelar_todas(cx));
        self.esperar(cx);
        self.app(cx, |app, _w, _cx| {
            assert_eq!(
                app.tela(),
                Tela::Revelacao,
                "o botão da barra abre a revelação"
            );
        });
    }

    /// Abre a Revelação na foto do site `id`, com a sessão inteira na tira.
    pub fn revelar_a_do_site(&self, cx: &mut TestAppContext, id: &str) {
        self.revelar_pela_barra(cx);
        let alvo = format!("site:{id}");
        self.revelacao(cx, |tela, window, cx| {
            let posicao = tela
                .acervo()
                .iter()
                .position(|f| f.id == alvo)
                .unwrap_or_else(|| panic!("a foto {alvo} não está na tira"));
            tela.ir_para(posicao, window, cx);
        });
        self.esperar(cx);
        self.revelacao(cx, |tela, _w, _cx| {
            assert_eq!(tela.foto_aberta().map(|f| f.id.clone()), Some(alvo));
            assert!(tela.tem_pixels(), "a cópia de trabalho está no cache");
        });
    }
}
