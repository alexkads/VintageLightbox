//! O estresse do caixa da galeria: um cupom de 400 fotos e a negociação em
//! lote, de quatro em quatro, com as respostas embaralhadas e falhas no meio.
//!
//! Mora dentro do painel (como `testes_do_painel.rs`) porque o lote é estado
//! dele. 🚨 **Só o `PublicadorDeMentira`**: dinheiro de cliente não passa por
//! aqui.

use std::sync::Arc;
use std::time::{Duration, Instant};

use domain::services::pos_venda::{EstadoDaFotoNoSite, FotoDaGaleria, GaleriaDoPainel, Sessao};
use gpui::{TestAppContext, WindowHandle};
use infrastructure::cache::preview_manager::PreviewManager;
use serde_json::{json, Value};

use super::*;
use crate::estresse::{cronometrar, relatar, Lcg};
use crate::importacao::explorador::mentira::ImportadorDeMentira;
use crate::pos_venda::porta::mentira::PublicadorDeMentira;
use crate::pos_venda::porta::Recado;
use crate::sessoes::arquivos::mentira::SeletorDeMentira;

const N: usize = 400;

fn foto_json(i: usize) -> Value {
    json!({
        "id": format!("f{i:04}"), "arquivo": format!("DSC_{i:04}.jpg"),
        "estado": "levada_no_balcao", "ordem": i, "nota": 4, "apagada_em": null,
        "produto_efetivo": if i.is_multiple_of(3) { "p2" } else { "p1" }, "produto_id": null,
        "preco_negociado": null, "observacao_da_negociacao": null
    })
}

fn galeria_json() -> Value {
    json!({
        "galeria": { "id": "g1", "titulo": "Casamento", "estudio_id": "e1" },
        "fotos": (0..N).map(foto_json).collect::<Vec<_>>(),
        "produto": { "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" },
        "produtos": [
            { "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" },
            { "id": "p2", "nome": "Impressa", "preco": "60.00", "preco_cheio": null }
        ],
        "avisos": []
    })
}

fn publicador() -> Arc<PublicadorDeMentira> {
    let p = Arc::new(PublicadorDeMentira {
        galerias: std::sync::Mutex::new(vec![GaleriaDoPainel {
            id: "g1".into(),
            titulo: "Casamento".into(),
            email: Some("noivos@x.com".into()),
            whatsapp: None,
            produto_id: "p1".into(),
            user_id: None,
            criada_em_iso: "2026-09-17".into(),
            expira_em: None,
            fotos: Default::default(),
            totais: None,
        }]),
        fotos_da_sessao: std::sync::Mutex::new(
            (0..N)
                .map(|i| FotoDaGaleria {
                    id: format!("f{i:04}"),
                    arquivo: format!("DSC_{i:04}.jpg"),
                    estado: EstadoDaFotoNoSite::LevadaNoBalcao,
                    ordem: i as i32,
                    preco_negociado: None,
                    observacao_da_negociacao: None,
                    apagada: false,
                    nota: Some(4),
                    produto_efetivo: "p1".into(),
                    preco_de_venda: None,
                    pedido_id: None,
                    downloads: 0,
                    revelada: false,
                    ajustes: None,
                })
                .collect(),
        ),
        // A rede segura as respostas: é o teste quem decide a ordem.
        demorada: true,
        ..Default::default()
    });
    p.responder_json(
        "funcionarios",
        Ok(json!([{ "id": "f1", "nome": "Ana", "ativo": true }])),
    );
    p.responder_json(
        "catalogo",
        Ok(json!([
            { "product": { "id": "p1", "name": "Digital", "price": "30.00", "normal_price": "40.00" } },
            { "product": { "id": "p2", "name": "Impressa", "price": "60.00", "normal_price": null } }
        ])),
    );
    p.responder_json("galeria", Ok(galeria_json()));
    p.responder_json("galeria-viva", Ok(galeria_json()));
    p.responder_json("vendas", Ok(json!([])));
    p.responder_json(
        "caixa",
        Ok(json!({
            "id": "cx", "estudio_id": "e1", "operador_email": "op@x",
            "fundo_de_troco_centavos": 0, "aberto_em": "2026-09-17T08:00:00Z",
            "movimentos": [], "vendas": [], "estornos": []
        })),
    );
    p.responder_json("lote", Ok(json!({})));
    p
}

struct Montagem {
    raiz: WindowHandle<gpui_component::Root>,
    caixa: Entity<Caixa>,
    detalhe: Entity<Detalhe>,
    publicador: Arc<PublicadorDeMentira>,
    _dir: tempfile::TempDir,
}

fn na_janela<R>(
    cx: &mut TestAppContext,
    m: &Montagem,
    f: impl FnOnce(&mut Caixa, &mut Window, &mut Context<Caixa>) -> R,
) -> R {
    let caixa = m.caixa.clone();
    m.raiz
        .update(cx, |_, window, cx| {
            caixa.update(cx, |t, cx| f(t, window, cx))
        })
        .expect("a janela aberta")
}

/// Solta o que a rede segurou **fora do lote** e deixa as telas colherem.
fn rede_sem_o_lote(cx: &mut TestAppContext, m: &Montagem) {
    for _ in 0..30 {
        let presos: Vec<_> =
            std::mem::take(&mut *m.publicador.guardados.lock().expect("guardados"));
        let (lote, outros): (Vec<_>, Vec<_>) = presos
            .into_iter()
            .partition(|(_, r)| matches!(r, Recado::Json { rotulo: "lote", .. }));
        m.publicador
            .guardados
            .lock()
            .expect("guardados")
            .extend(lote);
        for (canal, recado) in outros {
            let _ = canal.send(recado);
        }
        let detalhe = m.detalhe.clone();
        cx.update(|cx| {
            detalhe.update(cx, |d, cx| {
                d.colher(cx);
            })
        });
        na_janela(cx, m, |t, _, cx| {
            t.colher(cx);
        });
        cx.run_until_parked();
    }
}

fn montar(cx: &mut TestAppContext) -> Montagem {
    cx.update(gpui_component::init);
    let publicador = publicador();
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
    let mut guardados = None;
    let para_janela = publicador.clone();
    let raiz = cx.add_window(|window, cx| {
        let detalhe = cx.new(|cx| {
            let mut d = Detalhe::nova(
                para_janela.clone(),
                Arc::new(SeletorDeMentira::default()),
                Arc::new(ImportadorDeMentira::default()),
                previews,
                cx,
            );
            d.definir_sessao(Sessao {
                access_token: "tok".into(),
                refresh_token: "ref".into(),
                access_vence_em: i64::MAX,
                refresh_vence_em: i64::MAX,
            });
            d
        });
        let caixa = cx.new(|cx| Caixa::painel(para_janela.clone(), detalhe.clone(), window, cx));
        guardados = Some((caixa.clone(), detalhe));
        gpui_component::Root::new(caixa, window, cx)
    });
    let (caixa, detalhe) = guardados.expect("as telas criadas");
    let m = Montagem {
        raiz,
        caixa,
        detalhe,
        publicador,
        _dir: dir,
    };
    let detalhe = m.detalhe.clone();
    cx.update(|cx| detalhe.update(cx, |d, cx| d.entrar("g1".into(), cx)));
    rede_sem_o_lote(cx, &m);
    na_janela(cx, &m, |t, _, _| t.definir_visivel(true));
    rede_sem_o_lote(cx, &m);
    m
}

/// Os pedidos do lote já feitos: `PATCH /pos-venda/fotos/{id}`.
fn patches(m: &Montagem) -> Vec<String> {
    m.publicador
        .pedidos_json()
        .into_iter()
        .filter(|p| p.rotulo == "lote")
        .map(|p| p.caminho)
        .collect()
}

/// 🚨 **Cortesia nas 400 fotos do cupom: quatro no ar por vez, cada foto uma
/// vez, e o lote termina mesmo com respostas fora de ordem e falhas.**
#[gpui::test]
fn estresse_negociar_quatrocentas_de_quatro_em_quatro(cx: &mut TestAppContext) {
    let m = montar(cx);

    na_janela(cx, &m, |t, _, _| {
        let v = t.vista.as_ref().expect("o painel carregou");
        assert_eq!(v.cupom.itens.len(), N, "o cupom tem as 400");
        assert!(v.caixa.is_some(), "o caixa está aberto");
        cronometrar(
            "ler o cupom de 400 (×100)",
            Duration::from_millis(500),
            || {
                for _ in 0..100 {
                    assert_eq!(t.cupom().itens.len(), N);
                }
            },
        );
    });

    // Um quadro do painel com o cupom inteiro.
    let quadro = {
        let mut visual = gpui::VisualTestContext::from_window(m.raiz.into(), cx);
        let tamanho = gpui::size(gpui::px(1600.), gpui::px(1000.));
        visual.draw(gpui::Point::default(), tamanho, |_w, _cx| gpui::Empty);
        let inicio = Instant::now();
        for _ in 0..5 {
            let caixa = m.caixa.clone();
            m.raiz
                .update(&mut visual, |_, _w, cx| {
                    caixa.update(cx, |_t, cx| cx.notify())
                })
                .expect("a janela aberta");
            visual.draw(gpui::Point::default(), tamanho, |_w, _cx| gpui::Empty);
        }
        inicio.elapsed() / 10
    };
    relatar(
        "um quadro do painel com o cupom de 400",
        quadro,
        Duration::from_millis(100),
    );

    // Shift + ⌫ no cupom: a negociação de todos (o `negociar_rapido` de todos).
    na_janela(cx, &m, |t, _, cx| {
        t.negociar_rapido(Some(Negociacao::default()), true, cx);
        let lote = t.lote.as_ref().expect("o lote começou");
        assert_eq!(lote.total, N);
        assert_eq!(lote.no_ar, EM_VOO, "quatro no ar");
        assert_eq!(lote.fila.len(), N - EM_VOO);
        // Outro lote no meio não sai.
        t.mudar_tipo_de_ensaio(Some("p2".into()), true, cx);
        assert_eq!(
            t.lote.as_ref().map(|l| l.total),
            Some(N),
            "o segundo lote foi recusado"
        );
    });
    assert_eq!(patches(&m).len(), EM_VOO);

    let mut g = Lcg::novo(67);
    let mut falhas = 0usize;
    let mut respondidas = 0usize;
    let inicio = Instant::now();
    loop {
        let presos: Vec<_> =
            std::mem::take(&mut *m.publicador.guardados.lock().expect("guardados"));
        let (mut lote, outros): (Vec<_>, Vec<_>) = presos
            .into_iter()
            .partition(|(_, r)| matches!(r, Recado::Json { rotulo: "lote", .. }));
        for (canal, recado) in outros {
            let _ = canal.send(recado);
        }
        if lote.is_empty() {
            let no_ar = na_janela(cx, &m, |t, _, _| t.lote_no_ar());
            if !no_ar {
                break;
            }
            // Nada preso e o lote no ar: a colheita ainda não soltou os próximos.
            na_janela(cx, &m, |t, _, cx| {
                t.colher(cx);
            });
            cx.run_until_parked();
            assert!(
                !m.publicador.guardados.lock().expect("guardados").is_empty()
                    || !na_janela(cx, &m, |t, _, _| t.lote_no_ar()),
                "o lote está preso: no ar, e sem pedido nenhum na rede"
            );
            continue;
        }
        // A rede responde uma parte, fora de ordem; uma em sete falha.
        g.embaralhar(&mut lote);
        let quantas = 1 + g.ate(lote.len());
        let resto = lote.split_off(quantas);
        m.publicador
            .guardados
            .lock()
            .expect("guardados")
            .extend(resto);
        for (canal, mut recado) in lote {
            if g.ate(7) == 0 {
                recado = Recado::Json {
                    rotulo: "lote",
                    resultado: Err("500 Internal Server Error".into()),
                };
                falhas += 1;
            }
            respondidas += 1;
            let _ = canal.send(recado);
        }
        na_janela(cx, &m, |t, _, cx| {
            t.colher(cx);
            if let Some(lote) = t.lote.as_ref() {
                assert!(lote.no_ar <= EM_VOO, "{} no ar", lote.no_ar);
                assert!(lote.feitas + lote.no_ar + lote.fila.len() <= lote.total);
            }
        });
        cx.run_until_parked();
        assert!(respondidas <= N, "mais respostas que pedidos");
    }
    relatar(
        "negociar 400 em lote, com a rede embaralhada",
        inicio.elapsed(),
        Duration::from_secs(10),
    );

    let feitos = patches(&m);
    assert_eq!(feitos.len(), N, "um PATCH por foto");
    let unicos: std::collections::HashSet<_> = feitos.iter().collect();
    assert_eq!(unicos.len(), N, "nenhuma foto duas vezes");
    assert_eq!(respondidas, N);

    rede_sem_o_lote(cx, &m);
    na_janela(cx, &m, |t, _, cx| {
        assert!(t.lote.is_none(), "o lote terminou");
        let avisos = t.avisos_passageiros();
        if falhas > 0 {
            assert!(
                avisos
                    .iter()
                    .any(|a| a.starts_with(&format!("{falhas} não mudaram"))),
                "o aviso conta as {falhas} falhas: {avisos:?}"
            );
        }
        // E um lote novo sai, agora que o anterior acabou.
        t.mudar_tipo_de_ensaio(Some("p2".into()), true, cx);
        assert!(t.lote.is_some(), "um lote novo começou");
    });
    println!("ℹ️ {falhas} de {N} falharam no meio, e o lote terminou");
}
