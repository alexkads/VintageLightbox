//! O painel da galeria com a tela da sessão de verdade por trás — e o
//! `PublicadorDeMentira` no lugar da API: nenhuma gravação sai da máquina.

use std::sync::Arc;

use domain::services::pos_venda::{EstadoDaFotoNoSite, FotoDaGaleria, GaleriaDoPainel, Sessao};
use gpui_kit::{Entity, TestAppContext, WindowHandle};
use infrastructure::cache::preview_manager::PreviewManager;
use serde_json::{json, Value};

use super::*;
use crate::importacao::explorador::mentira::ImportadorDeMentira;
use crate::pos_venda::porta::mentira::PublicadorDeMentira;
use crate::sessoes::arquivos::mentira::SeletorDeMentira;
use biblioteca_core::negociacao::{Negociacao, Tipo};

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
    galeria_com(&["a", "b", "c"])
}

/// A galeria como o site a devolve, com estas fotos levadas e as outras à venda.
fn galeria_com(levadas: &[&str]) -> Value {
    let estado = |id: &str| {
        if levadas.contains(&id) {
            "levada_no_balcao"
        } else {
            "disponivel"
        }
    };
    json!({
        "galeria": { "id": "g1", "titulo": "Ensaio", "estudio_id": "e1" },
        "fotos": [
            foto_json("a", 0, estado("a")),
            foto_json("b", 1, estado("b")),
            foto_json("d", 2, estado("d")),
            foto_json("c", 3, estado("c")),
        ],
        "produto": { "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" },
        "produtos": [{ "id": "p1", "nome": "Digital", "preco": "30.00", "preco_cheio": "40.00" }],
        "avisos": []
    })
}

fn publicador() -> Arc<PublicadorDeMentira> {
    publicador_com(false)
}

/// Com `negociacao_demorada`, o `PATCH` da grade só volta em `responder()`.
fn publicador_com(negociacao_demorada: bool) -> Arc<PublicadorDeMentira> {
    let p = Arc::new(PublicadorDeMentira {
        negociacao_demorada,
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
            foto_do_site("c", 3, EstadoDaFotoNoSite::LevadaNoBalcao),
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
    raiz: WindowHandle<gpui_kit::component::Root>,
    caixa: Entity<Caixa>,
    detalhe: Entity<Detalhe>,
    publicador: Arc<PublicadorDeMentira>,
}

fn montar(cx: &mut TestAppContext) -> Montagem {
    montar_com(cx, publicador())
}

fn montar_com(cx: &mut TestAppContext, publicador: Arc<PublicadorDeMentira>) -> Montagem {
    cx.update(gpui_kit::init);
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
        gpui_kit::component::Root::new(caixa, window, cx)
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
    aberto_com(cx, publicador())
}

fn aberto_com(cx: &mut TestAppContext, publicador: Arc<PublicadorDeMentira>) -> Montagem {
    let m = montar_com(cx, publicador);
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

#[gpui_kit::test]
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
        assert_eq!(ids, vec!["a", "b", "c"]);
        assert_eq!(v.cupom.total, 12000);
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

#[gpui_kit::test]
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

#[gpui_kit::test]
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
        assert_eq!(t.lancados()[0].valor, 12000);
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

#[gpui_kit::test]
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

#[gpui_kit::test]
fn shift_apaga_a_negociacao_de_todos_os_itens(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        no_cupom(t, "down", false, w, cx);
        no_cupom(t, "e", false, w, cx);
        no_cupom(t, "backspace", true, w, cx);
    });
    colher(cx, &m);
    let caminhos: Vec<String> = gravacoes(&m).into_iter().map(|p| p.caminho).collect();
    assert_eq!(
        caminhos,
        vec![
            "/pos-venda/fotos/a",
            "/pos-venda/fotos/b",
            "/pos-venda/fotos/c"
        ]
    );
    na_janela(cx, &m, |t, _, _| {
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Negociação removida de 3 foto(s)."]
        );
    });
}

#[gpui_kit::test]
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

#[gpui_kit::test]
fn n_sem_item_negocia_o_cupom_inteiro_pelo_dialogo(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        assert!(no_cupom(t, "n", false, w, cx));
        assert_eq!(t.dialogo_aberto(), Some("Negociacao"));
        // O tipo padrão é cortesia: Enter salva.
        t.confirmar(w, cx);
    });
    colher(cx, &m);
    assert_eq!(gravacoes(&m).len(), 3);
    na_janela(cx, &m, |t, _, _| {
        assert_eq!(t.dialogo_aberto(), None);
        assert_eq!(
            t.avisos_passageiros(),
            vec!["3 foto(s) com a negociação registrada."]
        );
    });
}

const CTRL: Modificadores = Modificadores {
    aditivo: true,
    faixa: false,
};

/// ☑️ Dono, 2026-09-26: marcar vários itens com Ctrl/Shift ou a caixinha, e o
/// ajuste rápido age só neles — nem no item sozinho, nem no cupom inteiro.
#[gpui_kit::test]
fn ctrl_marca_dois_itens_e_a_cortesia_vai_so_para_eles(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        t.clicar_item("a", false, Modificadores::default(), w, cx);
        t.clicar_item("c", false, CTRL, w, cx);
        assert_eq!(t.marcados_no_cupom(), 2);
        assert!(no_cupom(t, "e", false, w, cx));
        assert!(no_cupom(t, "c", false, w, cx));
    });
    colher(cx, &m);
    let caminhos: Vec<String> = gravacoes(&m).into_iter().map(|p| p.caminho).collect();
    assert_eq!(caminhos, vec!["/pos-venda/fotos/a", "/pos-venda/fotos/c"]);
    na_janela(cx, &m, |t, _, _| {
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Negociação registrada em 2 fotos marcadas."]
        );
    });
}

#[gpui_kit::test]
fn a_caixinha_alterna_sem_desfazer_o_resto_e_n_negocia_os_marcados(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        t.clicar_item("b", true, Modificadores::default(), w, cx);
        t.clicar_item("c", true, Modificadores::default(), w, cx);
        t.clicar_item("a", true, Modificadores::default(), w, cx);
        t.clicar_item("a", true, Modificadores::default(), w, cx);
        assert_eq!(t.marcados_no_cupom(), 2);
        assert!(no_cupom(t, "n", false, w, cx));
        assert_eq!(t.dialogo_aberto(), Some("Negociacao"));
        t.confirmar(w, cx);
    });
    colher(cx, &m);
    let caminhos: Vec<String> = gravacoes(&m).into_iter().map(|p| p.caminho).collect();
    assert_eq!(caminhos, vec!["/pos-venda/fotos/b", "/pos-venda/fotos/c"]);
}

#[gpui_kit::test]
fn shift_seta_estende_e_esc_desmarca(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        no_cupom(t, "down", false, w, cx);
        assert_eq!(t.marcados_no_cupom(), 1);
        no_cupom(t, "down", true, w, cx);
        no_cupom(t, "down", true, w, cx);
        assert_eq!(t.marcados_no_cupom(), 3);
        assert_eq!(t.lote_do_ajuste().len(), 3);
        assert!(no_cupom(t, "escape", false, w, cx));
        assert_eq!(t.marcados_no_cupom(), 0);
        // Sem marcados, o ajuste volta a ser do item em foco.
        assert_eq!(t.lote_do_ajuste().len(), 1);
    });
}

#[gpui_kit::test]
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

fn abrir_negociacao_existente(t: &mut Caixa, w: &mut Window, cx: &mut Context<Caixa>) {
    t.abrir_dialogo_de_negociacao(
        vec!["a".into()],
        "Negociação desta foto".into(),
        Negociacao {
            preco: Some(0),
            ..Negociacao::default()
        },
        Some(4000),
        true,
        w,
        cx,
    );
}

/// 🗑️ "Remover negociação" pergunta antes, por cima do diálogo — o
/// `useConfirmacao` do site. `Esc` fecha só a pergunta e devolve ao
/// formulário; "Remover" apaga.
#[gpui_kit::test]
fn remover_a_negociacao_pergunta_e_esc_fecha_so_a_pergunta(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        abrir_negociacao_existente(t, w, cx);
        t.perguntar_se_remove_a_negociacao(w, cx);
        assert!(t.perguntando_se_remove());
        t.fechar_dialogo(w, cx);
        assert!(!t.perguntando_se_remove());
        assert_eq!(t.dialogo_aberto(), Some("Negociacao"));
    });
    assert!(gravacoes(&m).is_empty());
    na_janela(cx, &m, |t, w, cx| {
        t.perguntar_se_remove_a_negociacao(w, cx);
        t.confirmar(w, cx);
    });
    colher(cx, &m);
    let g = gravacoes(&m);
    assert_eq!(g.len(), 1, "{g:?}");
    assert_eq!(g[0].caminho, "/pos-venda/fotos/a");
    assert_eq!(
        g[0].corpo,
        Some(json!({ "preco_negociado": null, "observacao_da_negociacao": null }))
    );
}

/// 🏷️ "Já paga em outro site" grava o site escolhido na lista — o primeiro,
/// quando ninguém mexe nela, como o `Select` do site.
#[gpui_kit::test]
fn ja_paga_em_outro_site_grava_o_site_da_lista(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        abrir_negociacao_existente(t, w, cx);
        t.trocar_tipo_da_negociacao(Tipo::Parceiro, w, cx);
        t.confirmar(w, cx);
    });
    colher(cx, &m);
    let g = gravacoes(&m);
    assert_eq!(g.len(), 1, "{g:?}");
    assert_eq!(
        g[0].corpo,
        Some(json!({ "preco_negociado": null, "observacao_da_negociacao": "TchêOfertas" }))
    );
}

fn ids_no_cupom(cx: &mut TestAppContext, m: &Montagem) -> Vec<String> {
    na_janela(cx, m, |t, _, _| {
        let v = t.vista.as_ref().expect("o painel carregou");
        v.cupom.itens.iter().map(|i| i.foto_id.clone()).collect()
    })
}

/// `P` na grade, na foto `id`.
fn p_na_grade(cx: &mut TestAppContext, m: &Montagem, id: &str) {
    let detalhe = m.detalhe.clone();
    cx.update(|cx| {
        detalhe.update(cx, |d, cx| {
            d.focar_foto(id, cx);
            d.alternar_levada(cx);
        })
    });
    colher(cx, m);
}

/// 🛒 **O cupom acompanha o `P` da grade — o de marcar e o de desistir.**
///
/// A grade pinta a levada antes de o site responder, e o caixa relia a galeria
/// nesse instante: o `GET` chegava ao site antes do `PATCH` e trazia o estado
/// velho. Quando o `PATCH` voltava, a grade já estava pintada e não mudava de
/// novo, e o caixa não relia mais — o cupom ficava um gesto atrás (dono,
/// 27/set/2026: *"uma levada e caixa não foi atualizado"*, e o cliente desistiu
/// de uma foto e o caixa *"ainda constam duas fotos"*).
#[gpui_kit::test]
fn o_cupom_acompanha_o_p_da_grade_mesmo_com_o_patch_demorado(cx: &mut TestAppContext) {
    let m = aberto_com(cx, publicador_com(true));
    assert_eq!(ids_no_cupom(cx, &m), vec!["a", "b", "c"]);

    // Marca `d`: até o `PATCH` voltar, o site ainda não a tem como levada.
    p_na_grade(cx, &m, "d");
    m.publicador.responder();
    m.publicador
        .responder_json("galeria-viva", Ok(galeria_com(&["a", "b", "d", "c"])));
    colher(cx, &m);
    assert_eq!(ids_no_cupom(cx, &m), vec!["a", "b", "d", "c"]);

    // O cliente desiste de `a`: o mesmo gesto, o mesmo atraso.
    p_na_grade(cx, &m, "a");
    m.publicador.responder();
    m.publicador
        .responder_json("galeria-viva", Ok(galeria_com(&["b", "d", "c"])));
    colher(cx, &m);
    assert_eq!(ids_no_cupom(cx, &m), vec!["b", "d", "c"]);
}

// ── 🖥️ A tela cheia (`tela_cheia.rs`) ──────────────────────────────────────

fn com_as_pessoas(t: &mut Caixa) {
    t.pessoas = regras::Pessoas {
        fotografo: Some("f1".into()),
        atendente: Some("f1".into()),
        auxiliar: Some("f1".into()),
    };
}

/// `F10` e os formulários que o desenho seguinte monta.
fn entrar_na_tela_cheia(t: &mut Caixa, w: &mut Window, cx: &mut Context<Caixa>) {
    assert!(t.teclar_no_painel("f10", false, false, None, false, false, w, cx));
    assert!(t.tela_cheia());
    t.montar(w, cx);
}

#[gpui_kit::test]
fn f10_entra_e_sai_da_tela_cheia_e_a_escolha_fica_lembrada(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        assert!(!t.tela_cheia(), "o painel nasce compacto");
        entrar_na_tela_cheia(t, w, cx);
        // Pessoas, pagamento e gaveta montados juntos — sem diálogo.
        assert_eq!(
            t.formularios_montados(),
            vec!["Movimento", "Pagamento", "Pessoas"]
        );
        let lembrado = std::fs::read_to_string(&t.painel_ref().unwrap().lembranca).unwrap();
        assert!(lembrado.contains("\"tela_cheia\":true"), "{lembrado}");
        // Na volta ao painel nada da tela cheia vira diálogo aberto.
        assert!(t.teclar_no_painel("f10", false, false, None, false, false, w, cx));
        assert!(!t.tela_cheia());
        assert_eq!(t.dialogo_aberto(), None);
    });
}

#[test]
fn o_arquivo_de_antes_da_tela_cheia_abre_o_painel_compacto() {
    let antigo: Guardado = serde_json::from_str(r#"{"x":-10.0,"y":0.0,"minimizado":false}"#)
        .expect("o arquivo antigo ainda se lê");
    assert!(!antigo.tela_cheia);
}

#[gpui_kit::test]
fn na_tela_cheia_a_venda_sai_inteira_sem_dialogo(cx: &mut TestAppContext) {
    let m = aberto(cx);
    m.publicador.responder_json("venda", Ok(json!({})));
    na_janela(cx, &m, |t, w, cx| {
        com_as_pessoas(t);
        entrar_na_tela_cheia(t, w, cx);
        // F4 leva ao pagamento, que já estava na tela.
        t.tecla(4, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Pagamento"));
        assert!(!t.cupom_parado(), "sem valor lançado o cupom segue livre");
        assert!(t.teclar_no_painel("2", false, true, Some("Pagamento"), false, false, w, cx));
        t.lancar(w, cx);
        assert_eq!(t.lancados()[0].valor, 12000);
        // Um valor lançado não trava o cupom: tirar uma foto refaz a conta.
        assert!(!t.cupom_parado());
        t.concluir_venda(w, cx);
        assert!(t.cupom_parado(), "com a venda gravando o cupom espera");
    });
    colher(cx, &m);
    let vendas: Vec<PedidoJson> = gravacoes(&m)
        .into_iter()
        .filter(|p| p.rotulo == "venda")
        .collect();
    assert_eq!(vendas.len(), 1, "uma venda, uma gravação");
    na_janela(cx, &m, |t, w, cx| {
        // O pagamento concluído some, e o desenho seguinte monta outro em branco.
        t.montar(w, cx);
        assert!(t.ativar("Pagamento"));
        assert!(t.lancados().is_empty());
        assert!(!t.cupom_parado());
        assert!(t.tela_cheia(), "a venda não tira o caixa da tela cheia");
    });
}

#[gpui_kit::test]
fn na_tela_cheia_a_venda_nao_sai_sem_as_pessoas(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        entrar_na_tela_cheia(t, w, cx);
        assert!(t.ativar("Pagamento"));
        assert!(t.teclar_no_painel("1", false, true, Some("Pagamento"), false, false, w, cx));
        t.lancar(w, cx);
        t.concluir_venda(w, cx);
        let erro = t.erro_no_dialogo().expect("a recusa aparece no pagamento");
        assert!(erro.contains("quem fotografou"), "{erro}");
    });
    colher(cx, &m);
    assert!(gravacoes(&m).is_empty());
}

#[gpui_kit::test]
fn na_tela_cheia_as_teclas_f_levam_a_regiao_e_o_cupom_segue_livre(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        com_as_pessoas(t);
        entrar_na_tela_cheia(t, w, cx);
        for (tecla, vez) in [(3, "Pessoas"), (6, "Movimento"), (2, "Pagamento")] {
            t.tecla(tecla, w, cx);
            assert_eq!(t.dialogo_aberto(), Some(vez), "F{tecla}");
            assert_eq!(t.formularios_montados().len(), 3, "F{tecla} não abre nada");
        }
        // F7 leva às vendas, que não são formulário: ninguém perde a vez.
        t.tecla(7, w, cx);
        assert_eq!(t.formularios_montados().len(), 3);
        // Com os formulários à vista, as setas do cupom continuam valendo.
        assert!(no_cupom(t, "down", false, w, cx));
        assert_eq!(t.painel_ref().unwrap().em_foco.as_deref(), Some("a"));
    });
}

#[gpui_kit::test]
fn na_tela_cheia_a_negociacao_nao_derruba_o_pagamento(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        com_as_pessoas(t);
        entrar_na_tela_cheia(t, w, cx);
        assert!(t.ativar("Pagamento"));
        assert!(no_cupom(t, "down", false, w, cx));
        assert!(no_cupom(t, "n", false, w, cx));
        assert_eq!(t.dialogo_aberto(), Some("Negociacao"));
        assert_eq!(
            t.formularios_montados(),
            vec!["Movimento", "Negociacao", "Pagamento", "Pessoas"]
        );
        assert!(t.cupom_parado(), "negociando, o cupom espera");
        // Esc fecha só a negociação.
        t.fechar_dialogo(w, cx);
        assert_eq!(
            t.formularios_montados(),
            vec!["Movimento", "Pagamento", "Pessoas"]
        );
    });
}

#[gpui_kit::test]
fn na_tela_cheia_a_resposta_da_venda_acha_o_pagamento_fora_da_vez(cx: &mut TestAppContext) {
    let m = aberto(cx);
    m.publicador.responder_json("venda", Ok(json!({})));
    na_janela(cx, &m, |t, w, cx| {
        com_as_pessoas(t);
        entrar_na_tela_cheia(t, w, cx);
        assert!(t.ativar("Pagamento"));
        assert!(t.teclar_no_painel("2", false, true, Some("Pagamento"), false, false, w, cx));
        t.lancar(w, cx);
        t.concluir_venda(w, cx);
        // Enquanto a venda grava, o operador vai à gaveta.
        t.tecla(6, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Movimento"));
    });
    colher(cx, &m);
    na_janela(cx, &m, |t, w, cx| {
        // A venda entrou: o pagamento recomeça em branco, e a gaveta, que
        // estava na vez, continua montada.
        t.montar(w, cx);
        assert_eq!(
            t.formularios_montados(),
            vec!["Movimento", "Pagamento", "Pessoas"]
        );
        assert!(t.ativar("Pagamento"));
        assert!(t.lancados().is_empty(), "o lançado saiu com a venda");
    });
}

#[gpui_kit::test]
fn minimizar_a_tela_cheia_nao_deixa_dialogo_no_painel(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        entrar_na_tela_cheia(t, w, cx);
        assert!(t.teclar_no_painel("f9", false, false, None, false, false, w, cx));
        assert!(t.minimizado() && !t.tela_cheia());
        assert_eq!(t.dialogo_aberto(), None);
        // F9 de novo devolve a tela cheia: é o desenho escolhido.
        assert!(t.teclar_no_painel("f9", false, false, None, false, false, w, cx));
        assert!(t.tela_cheia());
    });
}

#[gpui_kit::test]
fn na_tela_cheia_o_cupom_vazio_nao_vira_venda(cx: &mut TestAppContext) {
    let p = publicador();
    p.responder_json("galeria", Ok(galeria_com(&[])));
    p.responder_json("galeria-viva", Ok(galeria_com(&[])));
    let m = aberto_com(cx, p);
    na_janela(cx, &m, |t, w, cx| {
        com_as_pessoas(t);
        entrar_na_tela_cheia(t, w, cx);
        assert!(t.ativar("Pagamento"));
        t.concluir_venda(w, cx);
        let erro = t.erro_no_dialogo().expect("a recusa aparece no pagamento");
        assert!(erro.contains("Nenhuma foto"), "{erro}");
    });
    colher(cx, &m);
    assert!(gravacoes(&m).is_empty());
}

#[gpui_kit::test]
fn ir_a_galeria_no_meio_da_venda_nao_perde_o_pagamento_lancado(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        com_as_pessoas(t);
        entrar_na_tela_cheia(t, w, cx);
        assert!(t.ativar("Pagamento"));
        assert!(t.teclar_no_painel("2", false, true, Some("Pagamento"), false, false, w, cx));
        t.lancar(w, cx);
        // F10: a galeria. O pagamento não vira diálogo, e não some.
        assert!(t.teclar_no_painel("f10", false, false, None, false, false, w, cx));
        assert!(!t.tela_cheia());
        assert_eq!(t.dialogo_aberto(), None);
        // F10 de novo: está tudo onde ficou.
        entrar_na_tela_cheia(t, w, cx);
        assert!(t.ativar("Pagamento"));
        assert_eq!(t.lancados().len(), 1);
        assert_eq!(t.lancados()[0].valor, 12000);
    });
    assert!(gravacoes(&m).is_empty());
}

#[gpui_kit::test]
fn na_tela_cheia_o_p_poe_e_tira_a_foto_do_cupom_sem_voltar_a_galeria(cx: &mut TestAppContext) {
    let m = aberto_com(cx, publicador_com(true));
    na_janela(cx, &m, entrar_na_tela_cheia);
    assert_eq!(ids_no_cupom(cx, &m), vec!["a", "b", "c"]);

    // O balconista clica em `d` na fila da sessão: a foto fica em foco…
    let detalhe = m.detalhe.clone();
    cx.update(|cx| {
        detalhe.update(cx, |d, cx| {
            assert!(d.so_esta("d", cx));
        })
    });
    colher(cx, &m);
    // …e o `P`, com o teclado no cupom, a leva ao balcão.
    na_janela(cx, &m, |t, w, cx| {
        assert_eq!(t.painel_ref().unwrap().em_foco.as_deref(), Some("d"));
        assert!(no_cupom(t, "p", false, w, cx));
    });
    m.publicador.responder();
    m.publicador
        .responder_json("galeria-viva", Ok(galeria_com(&["a", "b", "d", "c"])));
    colher(cx, &m);
    assert_eq!(ids_no_cupom(cx, &m), vec!["a", "b", "d", "c"]);

    // O mesmo `P` num item do cupom o tira.
    na_janela(cx, &m, |t, w, cx| {
        assert!(no_cupom(t, "p", false, w, cx));
    });
    m.publicador.responder();
    m.publicador
        .responder_json("galeria-viva", Ok(galeria_com(&["a", "b", "c"])));
    colher(cx, &m);
    assert_eq!(ids_no_cupom(cx, &m), vec!["a", "b", "c"]);
}

#[gpui_kit::test]
fn o_caixa_ve_todas_as_fotos_da_sessao_e_o_que_cada_uma_e(cx: &mut TestAppContext) {
    let m = aberto(cx);
    let fotos = cx.update(|cx| m.detalhe.read(cx).fotos_para_o_caixa());
    let ids: Vec<&str> = fotos.iter().map(|f| f.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b", "d", "c"], "na ordem da grade");
    let d = fotos.iter().find(|f| f.id == "d").unwrap();
    assert_eq!(d.estado, EstadoDaFotoNoSite::Disponivel);
    assert_eq!(d.nota, Some(4));
}

#[gpui_kit::test]
fn com_estorno_no_palco_o_cupom_espera_e_o_voltar_o_devolve(cx: &mut TestAppContext) {
    let m = aberto(cx);
    na_janela(cx, &m, |t, w, cx| {
        entrar_na_tela_cheia(t, w, cx);
        t.tecla(8, w, cx);
        assert!(t.formularios_montados().contains(&"Fechamento"));
        assert!(t.cupom_parado());
        // O "Voltar" do palco (e o Esc): só o fechamento sai.
        assert!(t.ativar("Fechamento"));
        t.fechar_dialogo(w, cx);
        assert!(!t.formularios_montados().contains(&"Fechamento"));
        assert!(!t.cupom_parado());
        assert!(t.tela_cheia());
    });
}
