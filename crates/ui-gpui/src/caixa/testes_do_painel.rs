//! O painel da galeria com a tela da sessão de verdade por trás — e o
//! `PublicadorDeMentira` no lugar da API: nenhuma gravação sai da máquina.

use std::sync::Arc;

use domain::services::pos_venda::{EstadoDaFotoNoSite, FotoDaGaleria, GaleriaDoPainel, Sessao};
use gpui::{Entity, TestAppContext, WindowHandle};
use infrastructure::cache::preview_manager::PreviewManager;
use serde_json::{json, Value};

use super::*;
use crate::importacao::explorador::mentira::ImportadorDeMentira;
use crate::pos_venda::porta::mentira::PublicadorDeMentira;
use crate::sessoes::arquivos::mentira::SeletorDeMentira;

fn foto_do_site(id: &str, ordem: i32, estado: EstadoDaFotoNoSite) -> FotoDaGaleria {
    FotoDaGaleria {
        id: id.into(),
        arquivo: format!("{id}.jpg"),
        estado,
        ordem,
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

fn galeria_json() -> Value {
    json!({
        "galeria": { "id": "g1", "titulo": "Ensaio", "estudio_id": "e1" },
        "fotos": [
            foto_json("a", 0, "levada_no_balcao"),
            foto_json("b", 1, "levada_no_balcao"),
            foto_json("d", 2, "disponivel"),
        ],
        "produto": { "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" },
        "produtos": [{ "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" }],
        "avisos": []
    })
}

fn publicador() -> Arc<PublicadorDeMentira> {
    let p = Arc::new(PublicadorDeMentira {
        galerias: std::sync::Mutex::new(vec![GaleriaDoPainel {
            id: "g1".into(),
            titulo: "Ensaio".into(),
            email: Some("ana@x.com".into()),
            whatsapp: None,
            produto_id: "p1".into(),
            user_id: None,
            criada_em_iso: "2026-09-06".into(),
            expira_em: None,
            fotos: Default::default(),
            totais: None,
            ..Default::default()
        }]),
        fotos_da_sessao: std::sync::Mutex::new(vec![
            foto_do_site("a", 0, EstadoDaFotoNoSite::LevadaNoBalcao),
            foto_do_site("b", 1, EstadoDaFotoNoSite::LevadaNoBalcao),
            foto_do_site("d", 2, EstadoDaFotoNoSite::Disponivel),
        ]),
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
            "fundo_de_troco_centavos": 0, "aberto_em": "2026-09-16T18:58:00Z",
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
}

fn montar(cx: &mut TestAppContext) -> Montagem {
    cx.update(gpui_component::init);
    let publicador = publicador();
    let dir = tempfile::TempDir::new().expect("diretório temporário");
    let previews = Arc::new(PreviewManager::new_with_path(dir.path().to_path_buf()));
    std::mem::forget(dir);
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
    };
    na_janela(cx, &m, |_, _, _| {});
    m
}

fn na_janela<R>(
    cx: &mut TestAppContext,
    m: &Montagem,
    f: impl FnOnce(&mut Caixa, &mut Window, &mut Context<Caixa>) -> R,
) -> R {
    let caixa = m.caixa.clone();
    let r = m
        .raiz
        .update(cx, |_, window, cx| {
            caixa.update(cx, |t, cx| f(t, window, cx))
        })
        .expect("a janela aberta");
    cx.run_until_parked();
    r
}

fn colher(cx: &mut TestAppContext, m: &Montagem) {
    for _ in 0..20 {
        let detalhe = m.detalhe.clone();
        cx.update(|cx| {
            detalhe.update(cx, |d, cx| {
                d.colher(cx);
            })
        });
        na_janela(cx, m, |t, _, cx| {
            t.colher(cx);
        });
    }
}

/// A galeria aberta e o painel carregado.
fn aberto(cx: &mut TestAppContext) -> Montagem {
    let m = montar(cx);
    let detalhe = m.detalhe.clone();
    cx.update(|cx| detalhe.update(cx, |d, cx| d.entrar("g1".into(), cx)));
    colher(cx, &m);
    na_janela(cx, &m, |t, _, _| t.definir_visivel(true));
    m
}

fn gravacoes(m: &Montagem) -> Vec<PedidoJson> {
    m.publicador
        .pedidos_json()
        .into_iter()
        .filter(|p| p.metodo != "GET")
        .collect()
}

/// Uma tecla no cupom, fora de campo de texto.
fn no_cupom(
    t: &mut Caixa,
    tecla: &str,
    shift: bool,
    w: &mut Window,
    cx: &mut Context<Caixa>,
) -> bool {
    t.teclar_no_painel(tecla, shift, false, None, true, false, w, cx)
}

#[gpui::test]
fn o_painel_segue_a_galeria_aberta_e_monta_o_cupom(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, _, _| {
        assert!(t.flutuante());
        assert_eq!(t.sessao_escolhida(), Some("g1"));
        let v = t.vista.as_ref().expect("o painel carregou");
        // O caixa é o do estúdio da sessão, e o preço é o de balcão.
        assert_eq!(v.estudio_id.as_deref(), Some("e1"));
        assert!(v.caixa.is_some());
        let ids: Vec<&str> = v.cupom.itens.iter().map(|i| i.foto_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
        assert_eq!(v.cupom.total, 8000);
    });
    // O painel não pediu lista de sessões nem estúdios: não são dele.
    let rotulos: Vec<&str> = m
        .publicador
        .pedidos_json()
        .iter()
        .map(|p| p.rotulo)
        .collect();
    assert!(!rotulos.contains(&"galerias") && !rotulos.contains(&"estudios"));
}

#[gpui::test]
fn f9_minimiza_e_as_letras_nao_chegam_a_grade(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        let antes = t.minimizado();
        assert!(t.teclar_no_painel("f9", false, false, None, false, false, w, cx));
        assert_eq!(t.minimizado(), !antes);
        // Com o foco no cupom, o `P` da grade não passa…
        assert!(no_cupom(t, "p", false, w, cx));
        // …mas com o foco na grade, o painel só fica com as teclas F.
        assert!(!t.teclar_no_painel("p", false, false, None, false, false, w, cx));
        // F2 abre o desconto: o caixa está aberto.
        assert!(t.teclar_no_painel("f2", false, false, None, false, false, w, cx));
        assert_eq!(t.dialogo_aberto(), Some("Desconto"));
        // Com o diálogo aberto, as outras teclas F esperam.
        t.teclar_no_painel("f7", false, true, Some("Desconto"), false, false, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Desconto"));
        // E o `%` troca o modo mesmo com o foco no campo.
        assert!(t.teclar_no_painel("5", true, true, Some("Desconto"), false, true, w, cx));
        assert_eq!(
            t.desconto.modo,
            regras::ModoDoDesconto::Valor,
            "só ao aplicar"
        );
    });
}

#[gpui::test]
fn no_pagamento_os_numeros_escolhem_a_forma_e_nao_dao_nota(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        t.pessoas = regras::Pessoas {
            fotografo: Some("f1".into()),
            atendente: Some("f1".into()),
            auxiliar: Some("f1".into()),
        };
        t.tecla(4, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Pagamento"));
        assert!(t.teclar_no_painel("2", false, true, Some("Pagamento"), false, false, w, cx));
        t.lancar(w, cx);
        assert_eq!(t.lancados()[0].forma, regras::FormaDePagamento::Pix);
        assert_eq!(t.lancados()[0].valor, 8000);
        assert!(t.teclar_no_painel(
            "backspace",
            false,
            true,
            Some("Pagamento"),
            false,
            false,
            w,
            cx
        ));
        assert!(t.lancados().is_empty());
    });
    assert!(gravacoes(&m).is_empty());
}

#[gpui::test]
fn a_edicao_rapida_grava_a_cortesia_e_a_galeria_se_rele(cx: &mut TestAppContext) {
    let m = aberto(cx);
    let aberturas_antes = m.publicador.abertas.lock().unwrap().len();
    na_janela(cx, &m, |t, w, cx| {
        // ↓ escolhe o primeiro item, E abre a barra, C dá cortesia.
        assert!(no_cupom(t, "down", false, w, cx));
        assert!(no_cupom(t, "e", false, w, cx));
        assert!(no_cupom(t, "c", false, w, cx));
    });
    colher(cx, &m);
    let g = gravacoes(&m);
    assert_eq!(g.len(), 1, "{g:?}");
    assert_eq!(g[0].metodo, "PATCH");
    assert_eq!(g[0].caminho, "/pos-venda/fotos/a");
    assert_eq!(
        g[0].corpo,
        Some(json!({ "preco_negociado": "0.00", "observacao_da_negociacao": "Cortesia" }))
    );
    na_janela(cx, &m, |t, _, _| {
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Negociação registrada em esta foto."]
        );
    });
    assert!(
        m.publicador.abertas.lock().unwrap().len() > aberturas_antes,
        "a galeria tem de se reler depois de gravar"
    );
    assert!(m
        .publicador
        .pedidos_json()
        .iter()
        .any(|p| p.rotulo == "galeria-viva"));
}

#[gpui::test]
fn shift_apaga_a_negociacao_de_todos_os_itens(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        no_cupom(t, "down", false, w, cx);
        no_cupom(t, "e", false, w, cx);
        no_cupom(t, "backspace", true, w, cx);
    });
    colher(cx, &m);
    let caminhos: Vec<String> = gravacoes(&m).into_iter().map(|p| p.caminho).collect();
    assert_eq!(caminhos, vec!["/pos-venda/fotos/a", "/pos-venda/fotos/b"]);
    na_janela(cx, &m, |t, _, _| {
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Negociação removida de 2 foto(s)."]
        );
    });
}

#[gpui::test]
fn o_tipo_de_ensaio_vai_para_a_foto(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        no_cupom(t, "down", false, w, cx);
        no_cupom(t, "e", false, w, cx);
        t.mudar_tipo_de_ensaio(Some("p2".into()), false, cx);
    });
    colher(cx, &m);
    let g = gravacoes(&m);
    assert_eq!(g[0].corpo, Some(json!({ "produto_id": "p2" })));
    na_janela(cx, &m, |t, _, _| {
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Tipo de ensaio alterado em esta foto."]
        );
    });
}

#[gpui::test]
fn n_sem_item_negocia_o_cupom_inteiro_pelo_dialogo(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        assert!(no_cupom(t, "n", false, w, cx));
        assert_eq!(t.dialogo_aberto(), Some("Negociacao"));
        // O tipo padrão é cortesia: Enter salva.
        t.confirmar(w, cx);
    });
    colher(cx, &m);
    assert_eq!(gravacoes(&m).len(), 2);
    na_janela(cx, &m, |t, _, _| {
        assert_eq!(t.dialogo_aberto(), None);
        assert_eq!(
            t.avisos_passageiros(),
            vec!["2 foto(s) com a negociação registrada."]
        );
    });
}

#[gpui::test]
fn sem_estudio_o_painel_diz_onde_escolher(cx: &mut TestAppContext) {
    let m = montar(cx);
    let mut sem = galeria_json();
    sem["galeria"]["estudio_id"] = Value::Null;
    m.publicador.responder_json("galeria", Ok(sem));
    let detalhe = m.detalhe.clone();
    cx.update(|cx| detalhe.update(cx, |d, cx| d.entrar("g1".into(), cx)));
    colher(cx, &m);
    na_janela(cx, &m, |t, w, cx| {
        assert!(t.vista.as_ref().unwrap().estudio_id.is_none());
        t.tecla(8, w, cx);
        assert_eq!(t.dialogo_aberto(), None);
        assert_eq!(t.avisos_passageiros(), vec![regras::SESSAO_SEM_ESTUDIO]);
    });
}
