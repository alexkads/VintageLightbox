//! 🎬 **O ciclo de vida de uma sessão, contra a API de verdade** — a garantia
//! de cada versão lançada (dono, 03/out/2026).
//!
//! Os outros cenários desta pasta falam com o [`super::PublicadorDeMentira`]:
//! provam a janela, e não a junta com o servidor. Este liga **a montagem do
//! balcão** (`crate::montagem::montar`, a mesma do `main.rs`) a uma API real, num
//! Postgres descartável — o `servidor-do-ciclo` do e-commerce — e percorre o
//! dia inteiro de uma sessão:
//!
//! | Ato | O que o operador faz | O que a API tem de dizer |
//! |---|---|---|
//! | 1 | cria a sessão e importa 6 fotos | a galeria, com estúdio, preço e as 6 fotos |
//! | 2 | mostra ao cliente na 2ª tela, dá notas, leva 3 no balcão, rejeita 1 | notas, levadas e a rejeitada fora |
//! | 3 | revela uma levada e salva na galeria | revelada, com os parâmetros, e o bruto intacto |
//! | 4 | abre o caixa, dá cortesia a uma, cobra PIX + dinheiro com troco | a venda pelo **preço cheio** |
//! | 5 | estorna uma foto e faz uma sangria | o estorno, a foto de volta à venda, o movimento |
//! | 6 | confere e fecha o caixa | o esperado de cada forma, sem diferença |
//! | 7 | manda o link; o cliente compra no pós-venda | o preço cheio, o pedido e as fotos liberadas |
//!
//! 🔑 **Rode pelo `make e2e-ciclo`.** Ele sobe o servidor, passa o endereço em
//! `VLB_E2E_CICLO` e um catálogo novo em `VLB_CATALOG`. Rodado à mão sem eles, o
//! cenário **falha** em vez de passar vazio — e nunca abre o catálogo de
//! verdade desta máquina.
//!
//! 🪟 Cada tecla e cada ato também exigem **o foco vivo** e **nenhum erro na
//! tela** (`Balcao::ato_limpo`): o caixa fechado derrubava o foco no campo da
//! Observação, e o cenário passava verde (03/out/2026).
//!
//! O que fica de mentira é só o que é janela do sistema (seletores de arquivo)
//! ou rede de terceiros (a atualização, o aviso do sistema).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use biblioteca_core::acervo::Estado;
use domain::services::pos_venda::Sessao;
use gpui_kit::component::Root;
use gpui_kit::{AppContext as _, Context, Entity, TestAppContext, Window, WindowHandle};
use serde_json::{json, Value};

use crate::app::{Aplicativo, Tela};
use crate::caixa::tela::Caixa;
use crate::revelacao::tela::Revelacao;
use crate::sessoes::detalhe::Detalhe;

mod tenebroso;

/// O prazo de cada espera: a rede, o disco e a GPU andam no relógio de verdade.
const PRAZO: Duration = Duration::from_secs(90);

/// O que o `servidor-do-ciclo` imprimiu.
struct Servidor {
    /// Onde o app fala: o proxy de falhas, na frente da API.
    api: String,
    /// A API sem o proxy — é por ela que o teste confere o que foi gravado.
    api_direta: String,
    /// A porta de controle do proxy de falhas.
    controle: String,
    aprovar: String,
    sessao: Sessao,
    funcionario: String,
    nome_do_funcionario: String,
}

fn ler_o_servidor() -> Servidor {
    let caminho = std::env::var("VLB_E2E_CICLO").unwrap_or_else(|_| {
        panic!(
            "🚨 VLB_E2E_CICLO não está definido: este cenário precisa da API de teste. \
             Rode `make e2e-ciclo` na raiz do VintageLightbox."
        )
    });
    let texto = std::fs::read_to_string(&caminho)
        .unwrap_or_else(|e| panic!("o arquivo do servidor ({caminho}) não abriu: {e}"));
    let v: Value = serde_json::from_str(texto.trim()).expect("a linha JSON do servidor");
    let texto_de = |v: &Value, chave: &str| {
        v[chave]
            .as_str()
            .unwrap_or_else(|| panic!("o servidor não disse `{chave}`: {v}"))
            .to_string()
    };
    let s = &v["sessao"];
    Servidor {
        api: texto_de(&v, "api"),
        api_direta: texto_de(&v, "api_direta"),
        controle: texto_de(&v, "controle"),
        aprovar: texto_de(&v, "aprovar"),
        sessao: Sessao {
            access_token: texto_de(s, "access_token"),
            refresh_token: texto_de(s, "refresh_token"),
            access_vence_em: s["access_vence_em"].as_i64().expect("access_vence_em"),
            refresh_vence_em: s["refresh_vence_em"].as_i64().expect("refresh_vence_em"),
        },
        funcionario: texto_de(&v["funcionarios"][0], "id"),
        nome_do_funcionario: texto_de(&v["funcionarios"][0], "nome"),
    }
}

/// Um cliente HTTP para afirmar o que a API gravou — o mesmo JSON que o painel
/// web e o navegador do cliente leem.
struct Http {
    tokio: tokio::runtime::Handle,
    cliente: reqwest::Client,
    base: String,
    token: Option<String>,
}

impl Http {
    fn novo(tokio: tokio::runtime::Handle, base: &str, token: Option<String>) -> Self {
        Self {
            tokio,
            cliente: reqwest::Client::new(),
            base: base.trim_end_matches('/').to_string(),
            token,
        }
    }

    fn pedido(&self, metodo: reqwest::Method, caminho: &str) -> reqwest::RequestBuilder {
        let url = if caminho.starts_with("http") {
            caminho.to_string()
        } else {
            format!("{}/api/v2{caminho}", self.base)
        };
        let pedido = self.cliente.request(metodo, url);
        match &self.token {
            Some(t) => pedido.bearer_auth(t),
            None => pedido,
        }
    }

    fn json(&self, pedido: reqwest::RequestBuilder) -> (u16, Value) {
        self.tokio.block_on(async move {
            let resposta = pedido.send().await.expect("a API respondeu");
            let status = resposta.status().as_u16();
            let texto = resposta.text().await.unwrap_or_default();
            (
                status,
                serde_json::from_str(&texto).unwrap_or(json!({ "bruto": texto })),
            )
        })
    }

    fn get(&self, caminho: &str) -> Value {
        let (status, corpo) = self.json(self.pedido(reqwest::Method::GET, caminho));
        assert_eq!(status, 200, "GET {caminho}: {corpo}");
        corpo
    }

    fn post(&self, caminho: &str, corpo: Value) -> (u16, Value) {
        self.json(self.pedido(reqwest::Method::POST, caminho).json(&corpo))
    }

    fn apagar(&self, caminho: &str) -> u16 {
        self.json(self.pedido(reqwest::Method::DELETE, caminho)).0
    }

    fn bytes(&self, caminho: &str) -> (u16, Vec<u8>) {
        let pedido = self.pedido(reqwest::Method::GET, caminho);
        self.tokio.block_on(async move {
            let resposta = pedido.send().await.expect("a API respondeu");
            let status = resposta.status().as_u16();
            (status, resposta.bytes().await.unwrap_or_default().to_vec())
        })
    }
}

/// O balcão montado: a janela, o app e o tokio das portas.
struct Balcao {
    raiz: WindowHandle<Root>,
    app: Entity<Aplicativo>,
    _tokio: tokio::runtime::Runtime,
}

impl Balcao {
    fn app<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Aplicativo, &mut Window, &mut Context<Aplicativo>) -> R,
    ) -> R {
        let app = self.app.clone();
        let r = cx
            .update_window(self.raiz.into(), |_raiz, window, cx| {
                app.update(cx, |app, cx| f(app, window, cx))
            })
            .expect("a janela principal está aberta");
        cx.run_until_parked();
        r
    }

    fn detalhe<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Detalhe, &mut Window, &mut Context<Detalhe>) -> R,
    ) -> R {
        self.app(cx, |app, window, cx| {
            let tela = app.detalhe.clone();
            tela.update(cx, |tela, cx| f(tela, window, cx))
        })
    }

    fn revelacao<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Revelacao, &mut Window, &mut Context<Revelacao>) -> R,
    ) -> R {
        self.app(cx, |app, window, cx| {
            let tela = app.revelacao.clone();
            tela.update(cx, |tela, cx| f(tela, window, cx))
        })
    }

    fn caixa<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Caixa, &mut Window, &mut Context<Caixa>) -> R,
    ) -> R {
        self.app(cx, |app, window, cx| {
            let caixa = app.caixa_flutuante.clone();
            caixa.update(cx, |caixa, cx| f(caixa, window, cx))
        })
    }

    /// Teclas de verdade, na janela principal.
    ///
    /// 🚨 Antes, confere que a tecla tem para onde ir ([`Self::foco_vivo`]) —
    /// a mesma régua do `teclar` dos outros cenários.
    fn teclar(&self, cx: &mut TestAppContext, teclas: &str) {
        self.foco_vivo(cx, &format!("antes da tecla {teclas}"));
        let mut visual = gpui_kit::VisualTestContext::from_window(self.raiz.into(), cx);
        visual.simulate_keystrokes(teclas);
        cx.run_until_parked();
    }

    /// 🪟 **O foco está vivo**: há um elemento focado, desenhado, e a rede da
    /// raiz nunca precisou apanhar um foco caído (`Aplicativo::foco_perdido`).
    /// Cada queda é uma sobreposição que fechou sem devolver o foco — o
    /// operador perde as teclas até clicar de novo.
    fn foco_vivo(&self, cx: &mut TestAppContext, onde: &str) {
        let visual = gpui_kit::VisualTestContext::from_window(self.raiz.into(), cx);
        visual.run_until_parked();
        self.app(cx, |app, window, cx| {
            assert_eq!(
                app.focos_perdidos,
                0,
                "🪟 {onde}: o foco caiu num elemento que sumiu (tela {:?}) — uma \
                 sobreposição fechou sem devolver o foco",
                app.tela()
            );
            use gpui_kit::component::WindowExt as _;
            let sobreposicao = window.has_active_dialog(cx) || window.has_active_sheet(cx);
            assert!(
                sobreposicao || app.foco_da_raiz().contains_focused(window, cx),
                "🪟 {onde}: nada desenhado tem o foco — as teclas não chegam a ninguém"
            );
        });
    }

    /// 🧾 **O ato terminou limpo**: o foco vivo, nenhum aviso de erro na tela,
    /// nenhuma recusa do servidor e a faixa de erro da sessão vazia. Um ato que
    /// chega ao fim com a API certa e um erro pintado na tela não passou.
    fn ato_limpo(&self, cx: &mut TestAppContext, ato: &str) {
        self.foco_vivo(cx, ato);
        let (erros, recusas, faixa) = self.app(cx, |app, _w, cx| {
            (
                app.avisos_dados_para_teste()
                    .into_iter()
                    .filter(|(_, erro)| *erro)
                    .map(|(texto, _)| texto)
                    .collect::<Vec<_>>(),
                app.recusas_para_teste().to_vec(),
                app.detalhe.read(cx).erro().map(|e| e.to_string()),
            )
        });
        assert!(erros.is_empty(), "🧾 {ato}: a tela avisou erro: {erros:?}");
        assert!(
            recusas.is_empty(),
            "🧾 {ato}: o servidor recusou: {recusas:?}"
        );
        assert!(
            faixa.is_none(),
            "🧾 {ato}: a faixa de erro da sessão: {faixa:?}"
        );
    }

    /// 🖱️ Clica no elemento marcado com `debug_selector`, onde o dedo clicaria.
    /// Um quadro novo antes de medir: o `debug_bounds` é o do último desenho.
    fn clicar(&self, cx: &mut TestAppContext, alvo: &str) {
        let mut visual = gpui_kit::VisualTestContext::from_window(self.raiz.into(), cx);
        visual.update(|window, _| window.refresh());
        visual.run_until_parked();
        let alvo: &'static str = Box::leak(alvo.to_string().into_boxed_str());
        let onde = visual
            .debug_bounds(alvo)
            .unwrap_or_else(|| panic!("🖱️ {alvo} não está desenhado na tela"));
        visual.simulate_click(onde.center(), gpui_kit::Modifiers::none());
        visual.run_until_parked();
        self.respirar(cx);
    }

    /// ⌨️ Digita no campo com o foco, como o sistema de entrada entrega o texto.
    fn digitar(&self, cx: &mut TestAppContext, texto: &str) {
        self.foco_vivo(cx, &format!("antes de digitar {texto:?}"));
        let mut visual = gpui_kit::VisualTestContext::from_window(self.raiz.into(), cx);
        visual.simulate_input(texto);
        visual.run_until_parked();
        self.respirar(cx);
    }

    /// 🔽 Uma lista de escolha (Select do kit), como o operador a usa: clica,
    /// anda uma opção com a seta e confirma com Enter, até `escolhido` devolver
    /// o valor certo. A lista reabre na opção já escolhida, então cada volta
    /// anda uma casa; no fim da lista, volta subindo.
    fn escolher_na_lista(
        &self,
        cx: &mut TestAppContext,
        alvo: &str,
        quer: &str,
        mut escolhido: impl FnMut(&Self, &mut TestAppContext) -> String,
    ) {
        let mut antes = escolhido(self, cx);
        for seta in ["down", "up"] {
            for _ in 0..40 {
                if antes == quer {
                    return;
                }
                self.clicar(cx, alvo);
                self.teclar(cx, seta);
                self.teclar(cx, "enter");
                self.respirar(cx);
                let agora = escolhido(self, cx);
                if agora == antes && !antes.is_empty() {
                    break; // a ponta da lista
                }
                antes = agora;
            }
        }
        assert_eq!(antes, quer, "🔽 a lista {alvo} não chegou à opção esperada");
    }

    /// ⬅️➡️ Anda pela grade da sessão com as setas até a foto `id` ficar em foco
    /// — o operador não "foca" uma foto, ele anda até ela.
    fn ir_na_grade(&self, cx: &mut TestAppContext, id: &str) {
        for _ in 0..40 {
            let (foco, ids) = self.detalhe(cx, |tela, _w, _cx| {
                (tela.em_foco().map(|f| f.id.clone()), tela.ids_visiveis())
            });
            if foco.as_deref() == Some(id) {
                return;
            }
            let alvo = ids
                .iter()
                .position(|i| i == id)
                .unwrap_or_else(|| panic!("⬅️➡️ a foto {id} não está na grade: {ids:?}"));
            let atual = foco.and_then(|f| ids.iter().position(|i| *i == f));
            let seta = match atual {
                Some(a) if a > alvo => "left",
                _ => "right",
            };
            self.teclar(cx, seta);
        }
        panic!("⬅️➡️ as setas não chegaram à foto {id}");
    }

    /// O relógio do teste anda (as colheitas das telas acordam a cada 100 ms)
    /// enquanto a rede, o disco e a GPU andam no relógio de verdade.
    fn respirar(&self, cx: &mut TestAppContext) {
        cx.executor().advance_clock(Duration::from_millis(150));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(25));
    }

    /// Espera `pronto` ficar verdadeiro, ou falha dizendo o que não aconteceu.
    fn ate(
        &self,
        cx: &mut TestAppContext,
        o_que: &str,
        mut pronto: impl FnMut(&Self, &mut TestAppContext) -> bool,
    ) {
        let fim = Instant::now() + PRAZO;
        loop {
            self.respirar(cx);
            if pronto(self, cx) {
                self.foco_vivo(cx, o_que);
                return;
            }
            if Instant::now() > fim {
                let (avisos, recusas, erro) = self.app(cx, |app, _w, cx| {
                    (
                        app.avisos_dados_para_teste(),
                        app.recusas_para_teste().to_vec(),
                        app.detalhe.read(cx).erro().map(|e| e.to_string()),
                    )
                });
                let do_caixa = self.caixa(cx, |c, _w, _cx| c.avisos_passageiros());
                panic!(
                    "⏱️ {o_que}: não aconteceu em {}s.\n  avisos: {avisos:?}\n  \
                     recusas: {recusas:?}\n  erro da sessão: {erro:?}\n  caixa: {do_caixa:?}",
                    PRAZO.as_secs()
                );
            }
        }
    }

    /// Como [`Self::ate`], mas consultando a API — a cada meio segundo, para
    /// não bater nela a cada respiro.
    fn ate_na_api(
        &self,
        cx: &mut TestAppContext,
        o_que: &str,
        mut pronto: impl FnMut() -> Result<(), String>,
    ) {
        let fim = Instant::now() + PRAZO;
        loop {
            for _ in 0..4 {
                self.respirar(cx);
            }
            let porque = match pronto() {
                Ok(()) => return,
                Err(porque) => porque,
            };
            assert!(
                Instant::now() < fim,
                "⏱️ {o_que}: a API não chegou lá em {}s — {porque}",
                PRAZO.as_secs()
            );
        }
    }
}

/// Abre o app como o `main.rs` abre: a montagem de verdade, num catálogo novo.
fn abrir_o_balcao(
    cx: &mut TestAppContext,
    servidor: &Servidor,
    cartao: &[String],
    rotulo: &str,
) -> Balcao {
    // 🚨 Nunca o catálogo desta máquina: sem `VLB_CATALOG`, nada abre. Cada
    // cenário tem o seu, dentro dele.
    let catalogo = std::env::var_os("VLB_CATALOG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!("🚨 VLB_CATALOG não está definido: o cenário não abre o catálogo de verdade")
        })
        .join(rotulo);
    std::fs::create_dir_all(&catalogo).expect("a pasta do catálogo");
    assert!(
        std::fs::read_dir(&catalogo)
            .expect("ler o catálogo")
            .next()
            .is_none(),
        "🚨 o catálogo do cenário tem de nascer vazio: {}",
        catalogo.display()
    );

    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        crate::tema::aplicar(crate::tema::Escolha::Claro, None, cx);
        crate::app::init(cx);
        crate::importacao::tela::init(cx);
        crate::cliente::init(cx);
    });

    let tokio = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("o tokio das portas");
    let api = Arc::new(infrastructure::PosVendaApiHttp::nova(servidor.api.clone()));
    let mut montagem = tokio.block_on(crate::montagem::montar(crate::montagem::Ambiente {
        catalogo: catalogo.clone(),
        banco: catalogo.join("vintage_lightbox.db"),
        previews: catalogo.join("previews"),
        api,
    }));
    // Só o que é janela do sistema ou rede de terceiros fica de mentira.
    let portas = &mut montagem.portas;
    portas.seletor = Arc::new(crate::importacao::explorador::mentira::SeletorDeMentira::default());
    portas.seletor_de_fotos =
        // A janela do sistema "escolhe" as fotos do cartão.
        Arc::new(crate::sessoes::arquivos::mentira::SeletorDeMentira::escolhe(
            &cartao.iter().map(String::as_str).collect::<Vec<_>>(),
        ));
    portas.escolha_de_presets = Arc::new(
        crate::revelacao::lightroom::mentira::EscolhaDeMentira::com(Vec::new()),
    );
    portas.escolha_do_backup =
        Arc::new(crate::backup::escolha::mentira::EscolhaDeMentira::default());
    portas.atualizador =
        Arc::new(crate::atualizacao::porta::mentira::AtualizadorDeMentira::default());
    portas.avisador = Arc::new(crate::tempo_real::aviso::mentira::AvisadorDeMentira::default());

    let crate::montagem::Montagem {
        fotos,
        presets,
        previews,
        edicoes,
        portas,
        ..
    } = montagem;
    let mut guardado = None;
    let raiz = cx.add_window({
        let guardado = &mut guardado;
        move |window, cx| {
            let app = cx.new(|cx| Aplicativo::novo(fotos, previews, presets, portas, window, cx));
            app.update(cx, |app, cx| app.definir_edicoes(edicoes, cx));
            app.read(cx).sessoes.read(cx).ja_respondeu_o_estudio();
            *guardado = Some(app.clone());
            Root::new(app, window, cx)
        }
    });
    raiz.update(cx, |_raiz, window, _cx| window.activate_window())
        .expect("a janela principal está aberta");
    cx.run_until_parked();
    Balcao {
        raiz,
        app: guardado.expect("o aplicativo foi construído"),
        _tokio: tokio,
    }
}

/// Seis fotos "do cartão", cada uma com conteúdo próprio — a importação pula
/// duplicata por hash.
fn gravar_o_cartao(pasta: &Path, quantas: u32) -> Vec<String> {
    std::fs::create_dir_all(pasta).expect("a pasta do cartão");
    (1..=quantas)
        .map(|n| {
            let mut imagem = image::RgbImage::new(1200, 800);
            for (x, y, pixel) in imagem.enumerate_pixels_mut() {
                *pixel = image::Rgb([
                    ((x / 4 + n * 37) % 256) as u8,
                    ((y / 3 + n * 53) % 256) as u8,
                    (60 + n * 25) as u8,
                ]);
            }
            let caminho = pasta.join(format!("DSC_{n:04}.jpg"));
            imagem.save(&caminho).expect("gravar o JPEG");
            caminho.to_string_lossy().to_string()
        })
        .collect()
}

/// As fotos vigentes da galeria no painel, pela ordem.
fn fotos_no_painel(site: &Http, galeria: &str) -> Vec<Value> {
    let aberta = site.get(&format!("/pos-venda/galerias/{galeria}"));
    let mut fotos: Vec<Value> = aberta["fotos"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|f| f["apagada_em"].is_null() && f["rejeitada_em"].is_null())
        .collect();
    fotos.sort_by_key(|f| f["ordem"].as_i64().unwrap_or(0));
    fotos
}

fn foto_no_painel(site: &Http, galeria: &str, id: &str) -> Option<Value> {
    fotos_no_painel(site, galeria)
        .into_iter()
        .find(|f| f["id"] == id)
}

/// Um decimal da API (`"40.00"` ou `40`) em centavos.
fn centavos(v: &Value) -> i64 {
    let n = match v {
        Value::String(s) => s.parse::<f64>().unwrap_or(f64::NAN),
        Value::Number(n) => n.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    assert!(n.is_finite(), "não é um valor: {v}");
    (n * 100.0).round() as i64
}

/// 🌩️ O controle do proxy de falhas do `servidor-do-ciclo`.
struct Falhas {
    http: Http,
    base: String,
}

impl Falhas {
    /// Programa uma falha (`metodo`, `caminho`, `modo`, `status`, `ms`, `vezes`).
    fn programar(&self, falha: Value) {
        let (status, corpo) = self.http.post(&format!("{}/falhas", self.base), falha);
        assert_eq!(status, 204, "o proxy não aceitou a falha: {corpo}");
    }

    /// Tira todas as falhas e zera o registro.
    fn limpar(&self) {
        assert_eq!(self.http.apagar(&format!("{}/falhas", self.base)), 204);
    }

    /// Cada pedido que passou pelo proxy: `metodo`, `caminho`, `feito`, `status`.
    fn registro(&self) -> Vec<Value> {
        self.http
            .get(&format!("{}/falhas/registro", self.base))
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// Quantas vezes o app pediu `metodo caminho` (o caminho contendo o trecho).
    fn quantas_vezes(&self, metodo: &str, caminho: &str) -> usize {
        self.registro()
            .iter()
            .filter(|p| {
                p["metodo"] == metodo && p["caminho"].as_str().is_some_and(|c| c.contains(caminho))
            })
            .count()
    }
}

/// 🎭 Um cenário montado: o balcão aberto e com a conta, a API para conferir, o
/// proxy de falhas e **o estúdio e o produto só dele** — o caixa de um cenário
/// nunca esbarra no de outro.
struct Cena {
    b: Balcao,
    site: Http,
    servidor: Servidor,
    tokio: tokio::runtime::Handle,
    falhas: Falhas,
    estudio_id: String,
    produto_id: String,
    _cartao: tempfile::TempDir,
}

/// Monta o cenário `rotulo` com `fotos` no cartão e entra na conta.
fn preparar(cx: &mut TestAppContext, rotulo: &str, fotos: u32) -> Cena {
    let servidor = ler_o_servidor();
    let cartao = tempfile::TempDir::new().expect("a pasta do cartão");
    let arquivos = gravar_o_cartao(cartao.path(), fotos);
    let b = abrir_o_balcao(cx, &servidor, &arquivos, rotulo);
    let tokio = b._tokio.handle().clone();
    let site = Http::novo(
        tokio.clone(),
        &servidor.api_direta,
        Some(servidor.sessao.access_token.clone()),
    );
    let falhas = Falhas {
        http: Http::novo(tokio.clone(), &servidor.controle, None),
        base: servidor.controle.clone(),
    };
    falhas.limpar();

    // O estúdio e o produto do cenário, antes de o app entrar na conta.
    let marca = format!(
        "{rotulo} {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros())
            .unwrap_or(0)
    );
    let (status, estudio) = site.post(
        "/bookings/studios",
        json!({
            "name": format!("Estúdio {marca}"), "address": "Rua do Teste, 1", "city": "Gramado",
            "businessHoursStart": "09:00", "businessHoursEnd": "18:00",
            "lunchStart": "12:00", "lunchEnd": "13:00", "checkInParallel": 1,
            "sessionDurationMinutes": 60, "intervalMinutes": 15,
            "timezone": "America/Sao_Paulo", "allowsWalkIn": false
        }),
    );
    assert!((200..300).contains(&status), "criar o estúdio: {estudio}");
    let (status, produto) = site.post(
        "/products",
        json!({
            "name": format!("Digital {marca}"),
            // 🔑 O pós-venda e o balcão cobram o cheio (normal_price), nunca o da loja.
            "price": 30.0, "normal_price": 40.0,
            "category_name": "Pós-venda", "inactive": true
        }),
    );
    assert!((200..300).contains(&status), "criar o produto: {produto}");

    b.app(cx, |app, _w, cx| {
        assert!(!app.entrou(), "o app abre na porta");
        // A conta entra pela autorização no navegador do sistema — a única
        // peça que não passa pela janela: a sessão vem pronta do servidor.
        app.entrar_na_conta(servidor.sessao.clone(), cx);
    });
    b.ate(
        cx,
        "a conta entra e a lista de sessões carrega",
        |b, cx| {
            b.app(cx, |app, _w, _cx| {
                app.entrou() && app.tela() == Tela::Sessoes
            })
        },
    );
    Cena {
        b,
        site,
        servidor,
        tokio,
        falhas,
        estudio_id: estudio["id"].as_str().expect("o id do estúdio").to_string(),
        produto_id: produto["product"]["id"]
            .as_str()
            .expect("o id do produto")
            .to_string(),
        _cartao: cartao,
    }
}

/// 🎬 Ato da criação: "Nova sessão" → "Escolher fotos" → título, e-mail, preço
/// e estúdio pela tela → "Criar". Confere a galeria na API e devolve o id dela
/// e os ids das fotos, pela ordem.
fn criar_a_sessao(
    cx: &mut TestAppContext,
    c: &Cena,
    titulo: &str,
    email: &str,
) -> (String, Vec<String>) {
    let b = &c.b;
    let quantas = c._cartao.path().read_dir().map(|d| d.count()).unwrap_or(0);
    b.clicar(cx, "sessoes-nova");
    b.ate(cx, "o assistente de nova sessão abre", |b, cx| {
        b.app(cx, |app, _w, _cx| app.tela() == Tela::NovaSessao)
    });
    // 📷 "Escolher fotos": a janela do sistema devolve as do cartão.
    b.clicar(cx, "nova-escolher-fotos");
    b.ate(cx, "as fotos do cartão entram no rascunho", |b, cx| {
        b.app(cx, |app, _w, cx| {
            app.nova_sessao.read(cx).quantas_fotos() == quantas
        })
    });
    b.clicar(cx, "nova-avancar");
    b.ate(cx, "a etapa do cliente e do preço", |b, cx| {
        b.app(cx, |app, _w, cx| app.nova_sessao.read(cx).etapa() == 3)
    });
    b.clicar(cx, "nova-titulo");
    b.digitar(cx, titulo);
    b.clicar(cx, "nova-email");
    b.digitar(cx, email);
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
    let f = formulario(b, cx);
    assert_eq!(
        (f.titulo.as_str(), f.email.as_str()),
        (titulo, email),
        "o que se digitou chegou ao formulário"
    );
    // Com tudo preenchido, o "Criar" do cabeçalho já vale na etapa 3.
    b.clicar(cx, "nova-criar-cabecalho");
    b.ate(cx, "a sessão é criada e o app entra nela", |b, cx| {
        b.app(cx, |app, _w, cx| {
            app.tela() == Tela::Sessao && app.detalhe.read(cx).galeria_id().is_some()
        })
    });
    let galeria = b
        .detalhe(cx, |tela, _w, _cx| tela.galeria_id().map(str::to_string))
        .expect("a galeria criada");

    let lista = c.site.get("/pos-venda/galerias");
    let na_lista = lista
        .as_array()
        .and_then(|l| l.iter().find(|g| g["id"] == galeria.as_str()))
        .unwrap_or_else(|| panic!("a sessão nova não está na lista do painel: {lista}"));
    assert_eq!(na_lista["titulo"], titulo);
    let aberta = c.site.get(&format!("/pos-venda/galerias/{galeria}"));
    assert_eq!(aberta["galeria"]["estudio_id"], c.estudio_id.as_str());
    assert_eq!(aberta["galeria"]["produto_id"], c.produto_id.as_str());
    assert_eq!(aberta["galeria"]["email"], email);

    b.ate_na_api(cx, "as fotos sobem ao site", || {
        let n = fotos_no_painel(&c.site, &galeria).len();
        (n == quantas)
            .then_some(())
            .ok_or(format!("{n} de {quantas}"))
    });
    let no_painel = fotos_no_painel(&c.site, &galeria);
    let fotos: Vec<String> = no_painel
        .iter()
        .map(|f| f["id"].as_str().expect("o id da foto").to_string())
        .collect();
    let mut ordens: Vec<i64> = no_painel
        .iter()
        .map(|f| f["ordem"].as_i64().unwrap_or(-1))
        .collect();
    ordens.dedup();
    assert_eq!(ordens.len(), quantas, "cada foto na sua ordem: {ordens:?}");
    b.ate(cx, "a grade da sessão mostra as fotos do site", |b, cx| {
        b.detalhe(cx, |tela, _w, _cx| {
            fotos.iter().all(|id| tela.como_esta(id).is_some())
        })
    });
    b.ato_limpo(cx, "a criação da sessão");
    (galeria, fotos)
}

/// ⭐🛍️ Dá a nota e leva no balcão (`B`) cada foto, pelas setas e teclas.
fn levar(cx: &mut TestAppContext, c: &Cena, galeria: &str, fotos: &[String]) {
    for foto in fotos {
        c.b.ir_na_grade(cx, foto);
        c.b.teclar(cx, "5");
        c.b.teclar(cx, "b");
    }
    c.b.ate_na_api(cx, "as fotos ficam levadas no balcão", || {
        let levadas: Vec<String> = fotos_no_painel(&c.site, galeria)
            .into_iter()
            .filter(|f| f["estado"] == "levada_no_balcao")
            .filter_map(|f| f["id"].as_str().map(str::to_string))
            .collect();
        fotos
            .iter()
            .all(|f| levadas.contains(f))
            .then_some(())
            .ok_or(format!("levadas: {levadas:?}"))
    });
}

/// 🧾 F8 com o caixa fechado: o fundo de troco e Enter. Devolve o id do caixa.
fn abrir_o_caixa(cx: &mut TestAppContext, c: &Cena, galeria: &str, fundo: &str) -> String {
    let b = &c.b;
    b.ate(cx, "o caixa flutuante lê a sessão", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa.sessao_escolhida() == Some(galeria) && !caixa.carregando()
        })
    });
    b.teclar(cx, "f8");
    b.caixa(cx, |caixa, _w, _cx| {
        assert_eq!(caixa.dialogo_do_caixa(), Some("Abrir"), "F8 abre o caixa")
    });
    b.digitar(cx, fundo);
    b.teclar(cx, "enter");
    b.ate(cx, "o caixa do estúdio abre", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| caixa.caixa_do_estudio_aberto())
    });
    let aberto = c
        .site
        .get(&format!("/pos-venda/caixa?estudio_id={}", c.estudio_id));
    aberto["id"].as_str().expect("o id do caixa").to_string()
}

/// 👥 F3: cada papel pela busca do nome e Enter; "Confirmar".
fn escolher_as_pessoas(cx: &mut TestAppContext, c: &Cena) {
    let b = &c.b;
    b.teclar(cx, "f3");
    b.caixa(cx, |caixa, _w, _cx| {
        assert_eq!(
            caixa.dialogo_do_caixa(),
            Some("Pessoas"),
            "F3 abre as pessoas"
        )
    });
    for papel in ["quem-fotografou", "quem-atendeu", "quem-auxiliou"] {
        b.clicar(cx, &format!("caixa-campo-{papel}"));
        b.digitar(cx, &c.servidor.nome_do_funcionario);
        b.teclar(cx, "enter");
    }
    b.clicar(cx, "caixa-pessoas-confirmar");
    b.ate(cx, "as três pessoas ficam gravadas", |b, cx| {
        b.caixa(cx, |caixa, _w, _cx| {
            caixa
                .pessoas_para_teste()
                .iter()
                .all(|p| p.as_deref() == Some(c.servidor.funcionario.as_str()))
        })
    });
}

/// As vendas da galeria no caixa, como o painel as lê.
fn vendas_da_galeria(c: &Cena, galeria: &str) -> Vec<Value> {
    c.site
        .get(&format!("/pos-venda/caixa/galerias/{galeria}/vendas"))
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// 🎬 **O ciclo de vida inteiro de uma sessão**, contra a API do
/// `servidor-do-ciclo`. Ver o cabeçalho do módulo.
#[gpui_kit::test]
#[ignore = "precisa da API de teste: rode `make e2e-ciclo`"]
fn o_ciclo_de_vida_da_sessao(cx: &mut TestAppContext) {
    let c = preparar(cx, "feliz", 6);
    let titulo = format!(
        "Ciclo de vida {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let email_do_cliente = "cliente-do-ciclo@e2e.test";

    // ── Ato 1: a criação da sessão ──────────────────────────────────────────
    let (galeria, fotos) = criar_a_sessao(cx, &c, &titulo, email_do_cliente);
    let Cena {
        b,
        site,
        servidor,
        tokio,
        estudio_id,
        _cartao,
        ..
    } = c;

    // ── Ato 2: a seleção com o cliente ──────────────────────────────────────
    b.ir_na_grade(cx, &fotos[0]);
    b.clicar(cx, "sessao-tela-do-cliente");
    b.ate(cx, "a segunda tela abre pelo botão", |b, cx| {
        b.app(cx, |app, _w, _cx| app.cliente_aberto())
    });
    let notas = [5u8, 4, 5, 3, 2, 1];
    for (foto, nota) in fotos.iter().zip(notas) {
        b.ir_na_grade(cx, foto);
        b.ate(cx, "a tela do cliente acompanha a foto", |b, cx| {
            b.app(cx, |app, _w, _cx| {
                app.parametros_no_cliente().map(|(id, _)| id) == Some(format!("site:{foto}"))
            })
        });
        b.teclar(cx, &nota.to_string());
    }
    b.ate_na_api(cx, "as notas chegam ao site", || {
        let lidas: Vec<Option<i64>> = fotos_no_painel(&site, &galeria)
            .iter()
            .map(|f| f["nota"].as_i64())
            .collect();
        let esperadas: Vec<Option<i64>> = notas.iter().map(|n| Some(*n as i64)).collect();
        (lidas == esperadas)
            .then_some(())
            .ok_or(format!("{lidas:?}"))
    });

    // As três primeiras ficam com o cliente (B); a última é rejeitada (X).
    for foto in &fotos[..3] {
        b.ir_na_grade(cx, foto);
        b.teclar(cx, "b");
    }
    b.ir_na_grade(cx, &fotos[5]);
    b.teclar(cx, "x");
    b.ate_na_api(
        cx,
        "3 levadas no balcão e a rejeitada fora do site",
        || {
            let vigentes = fotos_no_painel(&site, &galeria);
            let levadas = vigentes
                .iter()
                .filter(|f| f["estado"] == "levada_no_balcao")
                .count();
            let rejeitada_ainda = vigentes.iter().any(|f| f["id"] == fotos[5].as_str());
            (levadas == 3 && !rejeitada_ainda)
                .then_some(())
                .ok_or(format!(
                    "{levadas} levadas, rejeitada vigente: {rejeitada_ainda}"
                ))
        },
    );
    // Sem releitura forçada: a grade tem de se atualizar sozinha.
    b.ate(cx, "a sessão conta 3 levadas e 2 à venda", |b, cx| {
        b.detalhe(cx, |tela, _w, _cx| tela.contagem() == (3, 2, 0))
    });

    b.ato_limpo(cx, "Ato 2 (seleção)");

    // ── Ato 3: a revelação ──────────────────────────────────────────────────
    let revelada = fotos[0].clone();
    let (status, bruto_antes) = site.bytes(&format!("/pos-venda/fotos/{revelada}/original"));
    assert_eq!(status, 200, "o original da levada abre para o operador");
    b.ir_na_grade(cx, &revelada);
    b.clicar(cx, "sessao-revelar");
    b.ate(cx, "a Revelação abre pelo botão", |b, cx| {
        b.app(cx, |app, _w, _cx| app.tela() == Tela::Revelacao)
    });
    let alvo = format!("site:{revelada}");
    // As setas andam pela tira até a foto.
    for _ in 0..12 {
        let (aberta, ids) = b.revelacao(cx, |tela, _w, _cx| {
            (
                tela.foto_aberta().map(|f| f.id.clone()),
                tela.acervo()
                    .iter()
                    .map(|f| f.id.clone())
                    .collect::<Vec<_>>(),
            )
        });
        if aberta.as_deref() == Some(alvo.as_str()) {
            break;
        }
        let de = aberta.and_then(|a| ids.iter().position(|i| *i == a));
        let para = ids.iter().position(|i| *i == alvo).expect("a foto na tira");
        b.teclar(
            cx,
            if de.is_some_and(|d| d > para) {
                "left"
            } else {
                "right"
            },
        );
    }
    b.ate(cx, "a cópia de trabalho chega do site", |b, cx| {
        b.revelacao(cx, |tela, _w, _cx| {
            tela.foto_aberta().map(|f| f.id.as_str()) == Some(alvo.as_str()) && tela.tem_pixels()
        })
    });
    // A predefinição pelo clique no nome, na coluna.
    let predefinicao = b.revelacao(cx, |tela, _w, cx| {
        tela.coluna_de_predefinicoes(cx)
            .0
            .first()
            .cloned()
            .expect("há predefinições do sistema")
    });
    let antes_da_predefinicao = b.revelacao(cx, |tela, _w, _cx| tela.ajustes());
    b.clicar(cx, &format!("predefinicao-{predefinicao}"));
    let com_a_predefinicao = b.revelacao(cx, |tela, _w, _cx| tela.ajustes());
    assert_ne!(
        com_a_predefinicao, antes_da_predefinicao,
        "o clique em {predefinicao} muda a foto"
    );
    // A exposição pelo próprio evento do slider (arrastar com o mouse no
    // harness não é confiável; o caminho do evento é o mesmo do arrasto).
    b.revelacao(cx, |tela, _w, cx| tela.arrastar_slider(0, 0.6, cx));

    let ajustes = b.revelacao(cx, |tela, _w, _cx| tela.ajustes());
    assert!(
        (ajustes.exposure - 0.6).abs() < 1e-3,
        "a exposição por cima da predefinição {predefinicao}: {}",
        ajustes.exposure
    );
    b.ate(
        cx,
        "a tela do cliente vê a revelação ao vivo",
        |b, cx| {
            b.app(cx, |app, _w, _cx| {
                app.parametros_no_cliente()
                    .is_some_and(|(id, a)| id == alvo && (a.exposure - 0.6).abs() < 1e-3)
            })
        },
    );
    b.clicar(cx, "revelacao-salvar-na-galeria");
    b.ate(cx, "o Salvar volta para a sessão", |b, cx| {
        b.app(cx, |app, _w, _cx| app.tela() == Tela::Sessao)
    });
    b.ate_na_api(cx, "a foto fica revelada no site", || {
        let f = foto_no_painel(&site, &galeria, &revelada).ok_or("a foto sumiu")?;
        if f["revelada_em"].is_null() || f["revelacao_pendente"] == true {
            return Err(format!("ainda não: {f}"));
        }
        let exposicao = f["ajustes"]["exposure"].as_f64().unwrap_or(f64::NAN);
        ((exposicao - 0.6).abs() < 1e-3)
            .then_some(())
            .ok_or(format!("os parâmetros gravados: {}", f["ajustes"]))
    });
    b.ate(cx, "a fila de envios esvazia", |b, cx| {
        b.app(cx, |app, _w, _cx| app.a_subir_para_teste().is_empty())
    });
    let (_, bruto_depois) = site.bytes(&format!("/pos-venda/fotos/{revelada}/original"));
    assert!(
        bruto_antes == bruto_depois,
        "o bruto nunca muda: o editor reabre o mesmo original depois de revelar"
    );

    b.ato_limpo(cx, "Ato 3 (revelação)");

    // ── Ato 4: o pagamento no caixa ─────────────────────────────────────────
    b.ate(cx, "o caixa flutuante lê a sessão", |b, cx| {
        b.caixa(cx, |c, _w, _cx| {
            c.sessao_escolhida() == Some(galeria.as_str()) && !c.carregando()
        })
    });
    b.caixa(cx, |c, _w, _cx| {
        assert!(
            !c.caixa_do_estudio_aberto(),
            "o dia começa com o caixa fechado"
        )
    });
    b.teclar(cx, "f8");
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(c.dialogo_do_caixa(), Some("Abrir"), "F8 abre o caixa")
    });
    // O foco nasce no campo do fundo de troco: digita e Enter.
    b.digitar(cx, "100,00");
    b.teclar(cx, "enter");
    b.ate(
        cx,
        "o caixa do estúdio abre com R$ 100 de troco",
        |b, cx| b.caixa(cx, |c, _w, _cx| c.caixa_do_estudio_aberto()),
    );
    let caixa_aberto = site.get(&format!("/pos-venda/caixa?estudio_id={}", estudio_id));
    assert_eq!(
        caixa_aberto["fundo_de_troco_centavos"], 10000,
        "{caixa_aberto}"
    );
    let caixa_id = caixa_aberto["id"]
        .as_str()
        .expect("o id do caixa")
        .to_string();

    b.ate(cx, "o cupom tem as 3 levadas pelo preço cheio", |b, cx| {
        b.caixa(cx, |c, _w, _cx| {
            c.cupom_para_teste() == (fotos[..3].to_vec(), 12000)
        })
    });
    // A cortesia, pelo ajuste rápido do cupom: ↓ escolhe, E edita, C dá cortesia.
    // A cortesia: clica na linha da primeira levada no cupom, E abre o ajuste
    // rápido, C dá cortesia.
    b.clicar(cx, &format!("caixa-linha-{}", fotos[0]));
    b.teclar(cx, "e");
    b.teclar(cx, "c");
    let mut cortesia = String::new();
    b.ate_na_api(cx, "a cortesia fica gravada na foto", || {
        let com_cortesia: Vec<Value> = fotos_no_painel(&site, &galeria)
            .into_iter()
            .filter(|f| !f["preco_negociado"].is_null() && centavos(&f["preco_negociado"]) == 0)
            .collect();
        match com_cortesia.as_slice() {
            [f] => {
                cortesia = f["id"].as_str().unwrap_or_default().to_string();
                Ok(())
            }
            outras => Err(format!("{} fotos com cortesia", outras.len())),
        }
    });
    assert!(fotos[..3].contains(&cortesia), "a cortesia é numa levada");
    b.ate(cx, "o cupom cai para as duas cobradas", |b, cx| {
        b.caixa(cx, |c, _w, _cx| c.cupom_para_teste().1 == 8000)
    });

    // F3: quem fotografou e quem atendeu; F4: PIX 30 + dinheiro 60, troco 10.
    b.teclar(cx, "f3");
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(c.dialogo_do_caixa(), Some("Pessoas"), "F3 abre as pessoas")
    });
    // Cada papel: clica na lista, busca pelo nome e Enter.
    for papel in ["quem-fotografou", "quem-atendeu", "quem-auxiliou"] {
        b.clicar(cx, &format!("caixa-campo-{papel}"));
        b.digitar(cx, &servidor.nome_do_funcionario);
        b.teclar(cx, "enter");
    }
    b.clicar(cx, "caixa-pessoas-confirmar");
    b.ate(cx, "as três pessoas ficam gravadas", |b, cx| {
        b.caixa(cx, |c, _w, _cx| {
            c.pessoas_para_teste()
                .iter()
                .all(|p| p.as_deref() == Some(servidor.funcionario.as_str()))
        })
    });
    b.teclar(cx, "f4");
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(
            c.dialogo_do_caixa(),
            Some("Pagamento"),
            "F4 abre o pagamento"
        );
    });
    // 2 = PIX, o valor, Enter lança; 1 = dinheiro, o valor, Enter lança;
    // Enter de novo conclui.
    b.teclar(cx, "2");
    b.digitar(cx, "30,00");
    b.teclar(cx, "enter");

    b.teclar(cx, "1");
    b.digitar(cx, "60,00");
    b.teclar(cx, "enter");
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(c.pagamentos_lancados(), 2, "PIX e dinheiro lançados")
    });
    b.teclar(cx, "enter");
    b.ate(cx, "a venda é registrada", |b, cx| {
        b.caixa(cx, |c, _w, _cx| c.ultima_venda_para_teste().is_some())
    });
    let numero = b
        .caixa(cx, |c, _w, _cx| c.ultima_venda_para_teste())
        .expect("o número da venda");
    let no_caixa = site.get(&format!("/pos-venda/caixa/{caixa_id}"));
    let vendas = no_caixa["vendas"].as_array().cloned().unwrap_or_default();
    assert_eq!(vendas.len(), 1, "uma venda no caixa: {no_caixa}");
    let venda = &vendas[0];
    assert_eq!(venda["numero"], numero);
    assert_eq!(venda["galeria_id"], galeria.as_str());
    assert_eq!(
        venda["total_centavos"], 8000,
        "o balcão cobra o preço cheio (40,00), nunca o da loja (30,00): {venda}"
    );
    assert_eq!(venda["troco_centavos"], 1000, "{venda}");
    let mut vendidas: Vec<String> = venda["fotos_vendidas"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|f| f.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    vendidas.sort();
    let mut levadas = fotos[..3].to_vec();
    levadas.sort();
    assert_eq!(vendidas, levadas, "as três saem vendidas, a cortesia junto");

    b.ato_limpo(cx, "Ato 4 (caixa)");

    // ── Ato 5: o estorno e a sangria ────────────────────────────────────────
    let estornada = fotos[..3]
        .iter()
        .find(|f| **f != cortesia)
        .expect("uma levada cobrada")
        .clone();
    b.ate(cx, "a venda entra na lista da sessão (F7)", |b, cx| {
        b.caixa(cx, |c, _w, _cx| {
            c.vendas_da_sessao_para_teste()
                .iter()
                .any(|(n, total, _, _)| *n == numero && *total == 8000)
        })
    });
    b.teclar(cx, "f7");
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(c.dialogo_do_caixa(), Some("Vendas"), "F7 lista as vendas")
    });
    // "Estornar" na venda; desmarca as outras fotos (o valor acompanha), o
    // motivo, Enter.
    let venda_id = venda["id"].as_str().expect("o id da venda").to_string();
    b.clicar(cx, &format!("caixa-estornar-{venda_id}"));
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(
            c.dialogo_do_caixa(),
            Some("Estorno"),
            "o botão abre o estorno"
        )
    });
    let itens: Vec<String> = venda["itens"]
        .as_array()
        .map(|l| {
            l.iter()
                .filter_map(|i| i["foto_id"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for (i, foto) in itens.iter().enumerate() {
        if *foto != estornada {
            b.clicar(cx, &format!("caixa-estorno-foto-{i}"));
        }
    }
    b.clicar(cx, "caixa-campo-motivo");
    b.digitar(cx, "o cliente desistiu desta foto");
    b.teclar(cx, "enter");
    b.ate(cx, "a venda mostra o estorno", |b, cx| {
        b.caixa(cx, |c, _w, _cx| {
            c.vendas_da_sessao_para_teste()
                .iter()
                .any(|(n, _, estornado, _)| *n == numero && *estornado == 4000)
        })
    });
    b.ate_na_api(cx, "a foto estornada volta à venda", || {
        let f = foto_no_painel(&site, &galeria, &estornada).ok_or("a estornada sumiu")?;
        (f["estado"] == "disponivel")
            .then_some(())
            .ok_or(format!("estado {}", f["estado"]))
    });

    b.teclar(cx, "f6");
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(c.dialogo_do_caixa(), Some("Movimento"), "F6 abre a sangria")
    });
    // O foco nasce no valor; o motivo pelo clique; Enter registra.
    b.digitar(cx, "20,00");
    b.clicar(cx, "caixa-campo-motivo");
    b.digitar(cx, "depósito no banco");
    b.teclar(cx, "enter");
    b.ate_na_api(cx, "a sangria entra no caixa", || {
        let caixa = site.get(&format!("/pos-venda/caixa/{caixa_id}"));
        let n = caixa["movimentos"].as_array().map_or(0, Vec::len);
        (n == 1)
            .then_some(())
            .ok_or(format!("{n} movimentos: {caixa}"))
    });

    b.ato_limpo(cx, "Ato 5 (estorno)");

    // ── Ato 6: o fechamento ─────────────────────────────────────────────────
    // Dinheiro: 100 de fundo + 60 recebidos − 10 de troco − 40 estornados − 20
    // de sangria = 90. PIX: 30.
    b.ate(cx, "os diálogos fecharam", |b, cx| {
        b.caixa(cx, |c, _w, _cx| c.dialogo_do_caixa().is_none())
    });
    b.teclar(cx, "f8");
    b.caixa(cx, |c, _w, _cx| {
        assert_eq!(
            c.dialogo_do_caixa(),
            Some("Fechamento"),
            "F8 com o caixa aberto fecha"
        )
    });
    // A contagem cega: o foco nasce no dinheiro; o PIX pelo clique; Enter
    // confere, e Enter de novo fecha.
    b.digitar(cx, "90,00");
    b.clicar(cx, "caixa-campo-pix");
    b.digitar(cx, "30,00");
    b.teclar(cx, "enter");
    b.ate(cx, "a contagem cega é conferida", |b, cx| {
        b.caixa(cx, |c, _w, _cx| c.contagem_conferida())
    });
    b.teclar(cx, "enter");
    b.ate(cx, "o caixa fecha", |b, cx| {
        b.caixa(cx, |c, _w, _cx| c.caixa_fechado_no_dialogo())
    });
    let fechado = site.get(&format!("/pos-venda/caixa/{caixa_id}"));
    assert_eq!(
        fechado["esperado"]["dinheiro"], 9000,
        "o dinheiro esperado: 100 de fundo + 60 recebidos − 10 de troco − 40 estornados − 20 \
         de sangria = 90,00. O caixa: {fechado}"
    );
    assert_eq!(
        fechado["esperado"]["pix"], 3000,
        "o PIX esperado: os 30,00 da venda. O caixa: {fechado}"
    );
    for (forma, diferenca) in fechado["diferenca"].as_object().into_iter().flatten() {
        assert_eq!(diferenca, 0, "sem sobra nem falta em {forma}: {fechado}");
    }
    let agora = site.get(&format!("/pos-venda/caixa?estudio_id={}", estudio_id));
    assert!(agora.is_null(), "nenhum caixa aberto no estúdio: {agora}");
    b.teclar(cx, "escape");
    b.ate(cx, "o app vê o caixa fechado", |b, cx| {
        b.caixa(cx, |c, _w, _cx| !c.caixa_do_estudio_aberto())
    });

    b.ato_limpo(cx, "Ato 6 (fechamento)");

    // ── Ato 7: o pós-venda ──────────────────────────────────────────────────
    b.clicar(cx, "sessao-link");
    b.ate(cx, "o link do cliente chega", |b, cx| {
        b.detalhe(cx, |tela, _w, _cx| tela.link().is_some())
    });
    let link = b
        .detalhe(cx, |tela, _w, _cx| tela.link().map(|l| l.url.clone()))
        .expect("o link");
    let token = link
        .split("token=")
        .nth(1)
        .unwrap_or_else(|| panic!("o link entra sem senha: {link}"))
        .to_string();
    let anonimo = Http::novo(tokio.clone(), &servidor.api_direta, None);
    let (status, entrada) = anonimo.post("/auth/fast-link/resgatar", json!({ "token": token }));
    assert_eq!(status, 200, "o link abre a sessão do cliente: {entrada}");
    let cliente = Http::novo(
        tokio.clone(),
        &servidor.api_direta,
        entrada["access_token"].as_str().map(str::to_string),
    );
    let do_cliente = cliente.get(&format!("/meus-ensaios/{galeria}"));
    let fotos_do_cliente = do_cliente["fotos"].as_array().cloned().unwrap_or_default();
    let do_cliente_por_id = |id: &str| {
        fotos_do_cliente
            .iter()
            .find(|f| f["id"] == id)
            .cloned()
            .unwrap_or_else(|| panic!("o cliente não vê a foto {id}: {do_cliente}"))
    };
    assert!(
        fotos_do_cliente
            .iter()
            .all(|f| f["id"] != fotos[5].as_str()),
        "a rejeitada não chega ao cliente"
    );
    let a_venda: Vec<String> = [&estornada, &fotos[3], &fotos[4]]
        .iter()
        .map(|s| s.to_string())
        .collect();
    for id in &a_venda {
        let f = do_cliente_por_id(id);
        assert_eq!(f["estado"], "disponivel", "{f}");
        assert_eq!(
            centavos(&f["preco"]),
            4000,
            "o pós-venda cobra o preço cheio (40,00), nunca o da loja (30,00): {f}"
        );
    }
    let a_revelada = do_cliente_por_id(&revelada);
    assert_eq!(a_revelada["liberada"], true, "a levada já é do cliente");
    let (status, _) = cliente.bytes(&format!("/meus-ensaios/fotos/{revelada}/original"));
    assert_eq!(status, 200, "a levada no balcão baixa sem pagar de novo");
    let (status, _) = cliente.bytes(&format!("/meus-ensaios/fotos/{}/original", fotos[3]));
    assert_eq!(status, 403, "a que está à venda não baixa antes de pagar");

    let compradas = [fotos[3].clone(), fotos[4].clone()];
    let (status, pedido) = cliente.post(
        &format!("/meus-ensaios/{galeria}/comprar"),
        json!({ "fotos": compradas }),
    );
    assert_eq!(status, 201, "o cliente compra no pós-venda: {pedido}");
    let pedido_id = pedido["order_id"].as_str().expect("o pedido").to_string();
    let ordem = cliente.get(&format!("/orders/{pedido_id}"));
    assert_eq!(
        centavos(&ordem["total_amount"]),
        8000,
        "duas fotos pelo preço cheio: {ordem}"
    );
    let (status, _) = anonimo.post(&format!("{}/{pedido_id}", servidor.aprovar), json!({}));
    assert_eq!(status, 200, "o pagamento é aprovado");
    for id in &compradas {
        let (status, _) = cliente.bytes(&format!("/meus-ensaios/fotos/{id}/original"));
        assert_eq!(status, 200, "paga, a foto {id} baixa");
    }

    // O app vê a compra: as duas viram compradas na sessão.
    // 📡 Sem releitura forçada: o balcão fica sabendo da compra sozinho.
    b.ate(
        cx,
        "a compra do cliente chega à sessão aberta no balcão",
        |b, cx| {
            b.detalhe(cx, |tela, _w, _cx| {
                compradas
                    .iter()
                    .all(|id| tela.como_esta(id).map(|c| c.0) == Some(Estado::Comprada))
            })
        },
    );
    b.detalhe(cx, |tela, _w, _cx| {
        assert_eq!(
            tela.como_esta(&estornada).map(|c| c.0),
            Some(Estado::Disponivel),
            "a estornada segue à venda"
        );
    });
    b.ato_limpo(cx, "Ato 7 (pós-venda)");
}
