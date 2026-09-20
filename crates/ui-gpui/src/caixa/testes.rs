//! Os testes da tela do caixa, com um publicador que **nunca sai da máquina**.
//!
//! 🚨 O app de verdade fala com a API de produção, e o caixa grava dinheiro de
//! cliente. Toda gravação daqui vai para o [`PublicadorDoCaixa`], que anota o
//! pedido e responde com o JSON de exemplo — é aqui, e só aqui, que abrir,
//! vender, estornar e fechar são exercitados.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use biblioteca_core::caixa::{FormaDePagamento, Pessoas};
use domain::services::pos_venda::{
    EstadoNoBalcao, MudancaDaFoto, MudancaDaGaleria, NovaGaleria, Sessao,
};
use domain::value_objects::CropSettings;
use gpui::{Entity, TestAppContext, WindowHandle};
use infrastructure::gpu_adjustments::Ajustes;
use serde_json::{json, Value};

use super::*;
use crate::pos_venda::porta::FotoClassificada;

/// Um pedido que a tela fez.
#[derive(Debug, Clone)]
struct Pedido {
    metodo: &'static str,
    caminho: String,
    corpo: Option<Value>,
}

#[derive(Default)]
struct PublicadorDoCaixa {
    pedidos: Mutex<Vec<Pedido>>,
    /// O caixa aberto que o `GET /caixa` devolve (`null` = fechado).
    caixa: Mutex<Value>,
    /// Faz a leitura do caixa falhar.
    caixa_falha: bool,
}

fn venda_json(id: &str, numero: i64, fotos: &[&str], total: i64) -> Value {
    json!({
        "id": id, "numero": numero, "caixa_id": "cx", "galeria_id": "g1",
        "itens": fotos.iter().map(|f| json!({
            "foto_id": f, "ordem": 0, "arquivo": format!("{f}.jpg"), "faixa": "Digital",
            "cheio_centavos": 2500, "cobrado_centavos": 2500, "desconto_centavos": 0,
            "tipo_de_negociacao": null, "etiqueta": null, "estornada": false,
            "valor_sugerido_de_estorno_centavos": 2500
        })).collect::<Vec<_>>(),
        "pagamentos": [{ "forma": "pix", "valor_centavos": total, "detalhe": null }],
        "total_centavos": total, "troco_centavos": 0,
        "fotografo_id": "f1", "fotografo": "Ana", "atendente_id": "f1", "atendente": "Ana",
        "auxiliar_id": "f1", "auxiliar": "Ana",
        "criada_em": "2026-09-16T17:09:00Z",
        "estornado_centavos": 0, "estornavel_centavos": total,
        "fotos_vendidas": fotos, "estornos": []
    })
}

fn caixa_aberto() -> Value {
    json!({
        "id": "cx", "estudio_id": "e1", "operador_email": "op@estudio.com",
        "fundo_de_troco_centavos": 5000, "aberto_em": "2026-09-16T18:58:00Z",
        "contado": null, "esperado": null, "diferenca": null,
        "movimentos": [], "vendas": [], "estornos": []
    })
}

fn foto(id: &str, ordem: i64, estado: &str, negociado: Value, obs: Value) -> Value {
    json!({
        "id": id, "arquivo": format!("{id}.jpg"), "estado": estado, "ordem": ordem,
        "nota": 3, "apagada_em": null, "produto_efetivo": "p1",
        "preco_negociado": negociado, "observacao_da_negociacao": obs
    })
}

impl PublicadorDoCaixa {
    fn responder(&self, pedido: &PedidoJson) -> Result<Value, String> {
        let caminho = pedido.caminho.as_str();
        Ok(match (pedido.metodo, caminho) {
            ("GET", "/bookings/studios/admin") => json!([
                { "id": "e1", "name": "Centro Gramado", "city": "Gramado", "is_active": true },
                { "id": "e2", "name": "Canela", "city": "Canela", "is_active": true },
                { "id": "e3", "name": "Fechado", "city": "X", "is_active": false },
            ]),
            ("GET", "/pos-venda/galerias") => json!([
                { "id": "g0", "titulo": "Sem sinalizada", "email": "a@x", "whatsapp": null,
                  "estudio_id": "e1", "criada_em": "2026-09-16T12:00:00Z",
                  "fotos": { "levadas_no_balcao": 0 } },
                { "id": "g1", "titulo": "Ensaio da Maria", "email": null, "whatsapp": "5551",
                  "estudio_id": "e1", "criada_em": "2026-09-15T12:00:00Z",
                  "fotos": { "levadas_no_balcao": 3 } },
                { "id": "g2", "titulo": "De Canela", "email": null, "whatsapp": null,
                  "estudio_id": "e2", "criada_em": "2026-09-16T12:00:00Z",
                  "fotos": { "levadas_no_balcao": 1 } },
            ]),
            ("GET", "/pos-venda/funcionarios") => json!([
                { "id": "f1", "nome": "Ana", "whatsapp": null, "email": null, "ativo": true },
                { "id": "f2", "nome": "Saiu", "whatsapp": null, "email": null, "ativo": false },
            ]),
            ("GET", c) if c.starts_with("/products/admin") => json!([
                { "product": { "id": "p1", "name": "Digital", "price": "20.00", "normal_price": "25.00" } }
            ]),
            ("GET", "/pos-venda/galerias/g1") => json!({
                "galeria": { "id": "g1", "titulo": "Ensaio da Maria", "estudio_id": "e1" },
                "fotos": [
                    foto("a", 2, "levada_no_balcao", Value::Null, Value::Null),
                    foto("b", 1, "levada_no_balcao", json!("0.00"), json!("Cortesia")),
                    foto("c", 3, "levada_no_balcao", Value::Null, Value::Null),
                    foto("d", 4, "disponivel", Value::Null, Value::Null),
                ],
                "produto": { "id": "p1", "nome": "Digital", "preco": "20.00", "preco_cheio": "25.00" },
                "produtos": [{ "id": "p1", "nome": "Digital", "preco": "20.00", "preco_cheio": "25.00" }],
                "avisos": []
            }),
            ("GET", "/pos-venda/galerias/g2") => json!({
                "galeria": { "id": "g2", "titulo": "De Canela", "estudio_id": "e2" },
                "fotos": [], "produto": { "id": "p1", "nome": "Digital", "preco": "20.00" },
                "produtos": [], "avisos": []
            }),
            ("GET", "/pos-venda/galerias/sumiu") => {
                return Err("Erro de infraestrutura: o site respondeu 410: excluída".into())
            }
            // A foto `c` já foi vendida numa venda anterior.
            ("GET", c) if c.ends_with("/vendas") => json!([venda_json("v0", 1, &["c"], 2500)]),
            ("GET", c) if c.starts_with("/pos-venda/caixa?estudio_id=") => {
                if self.caixa_falha {
                    return Err("rede caída".into());
                }
                self.caixa.lock().unwrap().clone()
            }
            ("POST", "/pos-venda/caixa") => caixa_aberto(),
            ("POST", "/pos-venda/caixa/movimentos") => json!({
                "id": "m", "tipo": "sangria", "valor_centavos": 1, "motivo": "x",
                "criado_por": "op", "criado_em": "2026-09-16T19:00:00Z"
            }),
            ("POST", "/pos-venda/caixa/vendas") => {
                let mut v = venda_json("v9", 9, &["a"], 2500);
                v["troco_centavos"] = json!(2500);
                v
            }
            ("POST", c) if c.ends_with("/estornos") => venda_json("v0", 1, &[], 2500),
            ("PATCH", c) if c.starts_with("/pos-venda/fotos/") => json!({}),
            ("POST", "/pos-venda/caixa/conferir") => json!({
                "contado": { "dinheiro": 10000 },
                "esperado": { "dinheiro": 5000, "pix": 2500 },
                "diferenca": { "dinheiro": 5000, "pix": -2500 }
            }),
            ("POST", "/pos-venda/caixa/fechar") => {
                let mut c = caixa_aberto();
                c["contado"] = json!({ "dinheiro": 10000 });
                c["esperado"] = json!({ "dinheiro": 5000 });
                c["diferenca"] = json!({ "dinheiro": 5000 });
                c
            }
            ("POST", "/pos-venda/funcionarios") => json!({
                "id": "f9", "nome": "Nova", "whatsapp": null, "email": null, "ativo": true
            }),
            outro => return Err(format!("pedido inesperado: {outro:?}")),
        })
    }

    fn gravacoes(&self) -> Vec<Pedido> {
        self.pedidos
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.metodo != "GET")
            .cloned()
            .collect()
    }
}

impl Publicador for PublicadorDoCaixa {
    fn pedir_json(&self, _sessao: Sessao, pedido: PedidoJson, canal: Sender<Recado>) {
        self.pedidos.lock().unwrap().push(Pedido {
            metodo: pedido.metodo,
            caminho: pedido.caminho.clone(),
            corpo: pedido.corpo.clone(),
        });
        let _ = canal.send(Recado::Json {
            rotulo: pedido.rotulo,
            resultado: self.responder(&pedido),
        });
    }

    fn autorizar(&self, _: Sender<Recado>) {}
    fn retomar(&self, _: Sender<Recado>) {}
    fn sair(&self, _: Sender<Recado>) {}
    fn produtos(&self, _: Sessao, _: Sender<Recado>) {}
    fn estudios(&self, _: Sessao, _: Sender<Recado>) {}
    fn link(&self, _: Sessao, _: String, _: Sender<Recado>) {}
    fn atualizar_galeria(&self, _: Sessao, _: String, _: MudancaDaGaleria, _: Sender<Recado>) {}
    fn galerias(&self, _: Sessao, _: Sender<Recado>) {}
    fn criar_galeria(&self, _: Sessao, _: NovaGaleria, _: Sender<Recado>) {}
    fn subir_classificada(&self, _: Sessao, _: String, _: FotoClassificada, _: Sender<Recado>) {}
    fn tirar_do_site(&self, _: Sessao, _: String, _: Sender<Recado>) {}
    fn negociar(&self, _: Sessao, _: String, _: MudancaDaFoto, _: Sender<Recado>) {}
    fn enviar_arquivo(
        &self,
        _: Sessao,
        _: String,
        _: String,
        _: u32,
        _: EstadoNoBalcao,
        _: Sender<Recado>,
    ) {
    }
    fn abrir_galeria(&self, _: Sessao, _: String, _: Sender<Recado>) {}
    fn miniatura(&self, _: Sessao, _: String, _: Sender<Recado>) {}
    fn salvar_revelacao(
        &self,
        _: Sessao,
        _: String,
        _: Ajustes,
        _: CropSettings,
        _: Sender<Recado>,
    ) {
    }
    fn avisar(&self, _: Sessao, _: String, _: Sender<Recado>) {}
    fn copia_de_trabalho(&self, _: Sessao, _: String, _: String, _: Sender<Recado>) {}
}

/// A janela com o `Root` por baixo, como no app: o campo de texto do
/// `gpui-component` o procura ao ganhar o foco.
struct Janela {
    raiz: WindowHandle<gpui_component::Root>,
    tela: Entity<Caixa>,
}

fn janela(cx: &mut TestAppContext, publicador: Arc<PublicadorDoCaixa>) -> Janela {
    cx.update(gpui_component::init);
    let mut tela = None;
    let raiz = cx.add_window(|window, cx| {
        let caixa = cx.new(|cx| Caixa::nova(publicador, window, cx));
        tela = Some(caixa.clone());
        gpui_component::Root::new(caixa, window, cx)
    });
    let janela = Janela {
        raiz,
        tela: tela.expect("a tela criada"),
    };
    com(cx, &janela, |t, _, _| {
        t.definir_sessao(Sessao {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            access_vence_em: i64::MAX,
            refresh_vence_em: i64::MAX,
        })
    });
    janela
}

/// Faz a tela colher até não sobrar nada no ar.
fn colher(cx: &mut TestAppContext, janela: &Janela) {
    for _ in 0..20 {
        com(cx, janela, |t, _, cx| {
            t.colher(cx);
        });
    }
}

fn com<R>(
    cx: &mut TestAppContext,
    janela: &Janela,
    f: impl FnOnce(&mut Caixa, &mut Window, &mut Context<Caixa>) -> R,
) -> R {
    let tela = janela.tela.clone();
    let r = janela
        .raiz
        .update(cx, |_, window, cx| {
            tela.update(cx, |t, cx| f(t, window, cx))
        })
        .expect("a janela aberta");
    cx.run_until_parked();
    r
}

fn aberta_na_sessao(cx: &mut TestAppContext, publicador: &Arc<PublicadorDoCaixa>) -> Janela {
    *publicador.caixa.lock().unwrap() = caixa_aberto();
    let j = janela(cx, publicador.clone());
    com(cx, &j, |t, _, cx| {
        t.escolher_sessao(Some("g1".into()), cx);
        t.abrir(cx);
    });
    colher(cx, &j);
    j
}

#[gpui::test]
fn a_carga_escolhe_o_estudio_filtra_ordena_e_monta_o_cupom(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    com(cx, &j, |t, _, _| {
        let v = t.vista.as_ref().expect("a tela carregou");
        // Só os ativos; o estúdio é o da sessão pedida.
        assert_eq!(v.estudios.len(), 2);
        assert_eq!(v.estudio_id.as_deref(), Some("e1"));
        // Só as do estúdio, a com sinalizadas primeiro; o contato cai no WhatsApp.
        let ids: Vec<&str> = v.sessoes.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["g1", "g0"]);
        assert_eq!(v.sessoes[0].contato.as_deref(), Some("5551"));
        // `c` já foi vendida; `d` está à venda; `b` é cortesia. Preço de balcão.
        let itens: Vec<&str> = v.cupom.itens.iter().map(|i| i.foto_id.as_str()).collect();
        assert_eq!(itens, vec!["b", "a"]);
        assert_eq!(
            (v.cupom.subtotal, v.cupom.total, v.cupom.ja_vendidas),
            (5000, 2500, 1)
        );
        assert!(v.caixa.is_some() && !v.indisponivel);
        assert!(v.avisos.is_empty(), "{:?}", v.avisos);
        assert!(!t.carregando());
    });
}

#[gpui::test]
fn a_sessao_de_outro_estudio_e_a_excluida_viram_aviso(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = janela(cx, publicador.clone());
    // O estúdio pedido manda, e a sessão é de outro.
    com(cx, &j, |t, _, cx| {
        t.navegar(Some("e1".into()), Some("g2".into()), cx)
    });
    colher(cx, &j);
    com(cx, &j, |t, _, _| {
        let v = t.vista.as_ref().unwrap();
        assert!(v.sessao.is_none());
        assert_eq!(
            v.avisos,
            vec!["“De Canela” é de outro estúdio: escolha o estúdio dela para cobrar."]
        );
    });
    com(cx, &j, |t, _, cx| t.navegar(None, Some("sumiu".into()), cx));
    colher(cx, &j);
    com(cx, &j, |t, _, _| {
        let v = t.vista.as_ref().unwrap();
        assert_eq!(
            v.estudio_id.as_deref(),
            Some("e1"),
            "sem sessão, o primeiro"
        );
        assert_eq!(
            v.avisos,
            vec!["A sessão pedida não existe mais ou foi excluída."]
        );
    });
}

#[gpui::test]
fn o_caixa_que_nao_responde_fica_indisponivel_e_as_teclas_recusam(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa {
        caixa_falha: true,
        ..Default::default()
    });
    let j = janela(cx, publicador.clone());
    com(cx, &j, |t, _, cx| t.abrir(cx));
    colher(cx, &j);
    com(cx, &j, |t, w, cx| {
        assert!(t.vista.as_ref().unwrap().indisponivel);
        t.abrir_ou_fechar(w, cx);
        assert_eq!(t.dialogo_aberto(), None);
        assert_eq!(t.avisos_passageiros(), vec!["O caixa não respondeu."]);
    });
}

#[gpui::test]
fn f8_com_o_caixa_fechado_abre_e_grava_o_fundo(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = janela(cx, publicador.clone());
    com(cx, &j, |t, _, cx| t.abrir(cx));
    colher(cx, &j);
    com(cx, &j, |t, w, cx| {
        // F2 com o caixa fechado também cai no "abrir".
        t.com_caixa(TipoDeDialogo::Desconto, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Abrir"));
        t.fechar_dialogo(w, cx);
        t.abrir_ou_fechar(w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Abrir"));
        t.preencher("fundo", "abc", w, cx);
        t.confirmar(w, cx);
    });
    assert!(publicador.gravacoes().is_empty(), "valor inválido não sai");
    com(cx, &j, |t, w, cx| {
        t.preencher("fundo", "200,00", w, cx);
        t.confirmar(w, cx);
        // Enter duas vezes não abre dois caixas.
        t.confirmar(w, cx);
    });
    colher(cx, &j);
    let gravadas = publicador.gravacoes();
    assert_eq!(gravadas.len(), 1);
    assert_eq!(gravadas[0].caminho, "/pos-venda/caixa");
    assert_eq!(
        gravadas[0].corpo,
        Some(json!({ "estudio_id": "e1", "fundo_de_troco_centavos": 20000 }))
    );
    com(cx, &j, |t, _, _| {
        assert_eq!(t.dialogo_aberto(), None);
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Caixa aberto com R$ 200,00 de fundo de troco."]
        );
    });
}

#[gpui::test]
fn f4_pede_os_nomes_antes_e_a_venda_sai_como_o_backend_espera(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    com(cx, &j, |t, w, cx| {
        // O trio guardado com quem saiu do cadastro não vale.
        t.pessoas = Pessoas {
            fotografo: Some("f2".into()),
            atendente: Some("f1".into()),
            auxiliar: Some("f1".into()),
        };
        t.finalizar(w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Pessoas"));
        assert!(t.depois_das_pessoas);
        t.fechar_dialogo(w, cx);

        t.pessoas.fotografo = Some("f1".into());
        // Desconto no total de 10%.
        t.desconto = DescontoNoTotal {
            modo: ModoDoDesconto::Percentual,
            texto: "10".into(),
            motivo: " fidelidade ".into(),
        };
        assert_eq!(t.a_receber(), 2250);
        t.finalizar(w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Pagamento"));

        // Nada lançado: não conclui.
        t.concluir_venda(w, cx);
        t.escolher_forma_de_pagamento(FormaDePagamento::Pix, w, cx);
        t.preencher("valor", "10,00", w, cx);
        t.lancar(w, cx);
        t.escolher_forma_de_pagamento(FormaDePagamento::Dinheiro, w, cx);
        t.preencher("valor", "50,00", w, cx);
        t.confirmar(w, cx);
        assert_eq!(t.lancados().len(), 2);
        // Com tudo lançado, o Enter conclui.
        t.confirmar(w, cx);
    });
    colher(cx, &j);
    let gravadas = publicador.gravacoes();
    assert_eq!(gravadas.len(), 1, "{gravadas:?}");
    let corpo = gravadas[0].corpo.clone().unwrap();
    assert_eq!(gravadas[0].caminho, "/pos-venda/caixa/vendas");
    assert_eq!(corpo["galeria_id"], "g1");
    assert_eq!(corpo["desconto_no_total_centavos"], 250);
    assert_eq!(corpo["motivo_do_desconto"], "fidelidade");
    assert_eq!(corpo["itens"][0]["foto_id"], "b");
    assert_eq!(corpo["itens"][0]["tipo_de_negociacao"], "cortesia");
    assert_eq!(corpo["itens"][1]["cobrado_centavos"], 2500);
    assert_eq!(
        corpo["pagamentos"],
        json!([
            { "forma": "pix", "valor_centavos": 1000, "detalhe": null },
            { "forma": "dinheiro", "valor_centavos": 5000, "detalhe": null },
        ])
    );
    assert_eq!(corpo["fotografo_id"], "f1");
    com(cx, &j, |t, _, _| {
        assert_eq!(t.dialogo_aberto(), None);
        assert_eq!(t.ultima_venda.as_ref().map(|v| v.numero), Some(9));
        assert_eq!(t.desconto, DescontoNoTotal::default());
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Venda #9 registrada · R$ 25,00 · troco R$ 25,00"]
        );
    });
}

#[gpui::test]
fn a_sangria_confere_e_grava(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    com(cx, &j, |t, w, cx| {
        t.com_caixa(TipoDeDialogo::Movimento, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Movimento"));
        t.preencher("valor", "30,00", w, cx);
        t.confirmar(w, cx);
    });
    assert!(publicador.gravacoes().is_empty(), "sem motivo não sai");
    com(cx, &j, |t, w, cx| {
        t.preencher("motivo", "depósito", w, cx);
        t.confirmar(w, cx);
    });
    colher(cx, &j);
    assert_eq!(
        publicador.gravacoes()[0].corpo,
        Some(json!({
            "estudio_id": "e1", "tipo": "sangria", "valor_centavos": 3000, "motivo": "depósito"
        }))
    );
    com(cx, &j, |t, _, _| {
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Sangria de R$ 30,00 registrado."]
        );
    });
}

#[gpui::test]
fn o_estorno_devolve_e_des_sinaliza_foto_a_foto(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    com(cx, &j, |t, w, cx| {
        let venda = t.vista.as_ref().unwrap().sessao.as_ref().unwrap().vendas[0].clone();
        t.abrir_estorno(venda, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Estorno"));
        // Sem motivo não sai.
        t.confirmar(w, cx);
        t.preencher("motivo", "desistiu", w, cx);
        t.confirmar(w, cx);
    });
    colher(cx, &j);
    let gravadas = publicador.gravacoes();
    assert_eq!(gravadas.len(), 2, "{gravadas:?}");
    assert_eq!(gravadas[0].caminho, "/pos-venda/caixa/vendas/v0/estornos");
    assert_eq!(
        gravadas[0].corpo,
        Some(json!({
            "fotos": ["c"], "valor_centavos": 2500, "motivo": "desistiu",
            "pagamentos": [{ "forma": "pix", "valor_centavos": 2500, "detalhe": null }]
        }))
    );
    assert_eq!(gravadas[1].metodo, "PATCH");
    assert_eq!(gravadas[1].caminho, "/pos-venda/fotos/c");
    assert_eq!(gravadas[1].corpo, Some(json!({ "estado": "disponivel" })));
    com(cx, &j, |t, _, _| {
        assert_eq!(t.dialogo_aberto(), None);
        assert_eq!(
            t.avisos_passageiros(),
            vec!["Estorno de R$ 25,00 na venda #1 · 1 foto(s) de volta a “à venda”"]
        );
    });
}

#[gpui::test]
fn o_estorno_sem_foto_escolhida_nao_sai(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    com(cx, &j, |t, w, cx| {
        let venda = t.vista.as_ref().unwrap().sessao.as_ref().unwrap().vendas[0].clone();
        t.abrir_estorno(venda, w, cx);
        t.alternar_foto_do_estorno("c".into(), w, cx);
        t.preencher("motivo", "desistiu", w, cx);
        t.confirmar(w, cx);
    });
    colher(cx, &j);
    assert!(publicador.gravacoes().is_empty());
}

#[gpui::test]
fn o_fechamento_conta_as_cegas_confere_e_so_depois_fecha(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    com(cx, &j, |t, w, cx| {
        t.abrir_ou_fechar(w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Fechamento"));
        t.preencher("dinheiro", "100,00", w, cx);
        t.confirmar(w, cx);
    });
    colher(cx, &j);
    let gravadas = publicador.gravacoes();
    assert_eq!(gravadas.len(), 1, "conferir não fecha");
    assert_eq!(gravadas[0].caminho, "/pos-venda/caixa/conferir");
    assert_eq!(
        gravadas[0].corpo,
        Some(json!({ "estudio_id": "e1", "contado": { "dinheiro": 10000 } }))
    );
    com(cx, &j, |t, w, cx| t.confirmar(w, cx));
    colher(cx, &j);
    let gravadas = publicador.gravacoes();
    assert_eq!(gravadas.len(), 2);
    assert_eq!(gravadas[1].caminho, "/pos-venda/caixa/fechar");
    assert_eq!(
        gravadas[1].corpo,
        Some(json!({ "estudio_id": "e1", "contado": { "dinheiro": 10000 }, "observacao": null }))
    );
    // O resultado fica na tela; outro Enter não grava de novo.
    com(cx, &j, |t, w, cx| {
        assert_eq!(t.dialogo_aberto(), Some("Fechamento"));
        t.confirmar(w, cx);
    });
    colher(cx, &j);
    assert_eq!(publicador.gravacoes().len(), 2);
}

#[gpui::test]
fn as_teclas_f_esperam_com_dialogo_aberto(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    com(cx, &j, |t, w, cx| {
        t.tecla(1, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Atalhos"));
        t.tecla(8, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Atalhos"));
        t.fechar_dialogo(w, cx);
        t.tecla(7, w, cx);
        assert_eq!(t.dialogo_aberto(), Some("Vendas"));
        // A tela sumiu e voltou: o diálogo não fica pendurado.
        t.abrir(cx);
        assert_eq!(t.dialogo_aberto(), None);
    });
}

#[gpui::test]
fn abrir_sessao_pede_a_galeria_a_raiz(cx: &mut TestAppContext) {
    let publicador = Arc::new(PublicadorDoCaixa::default());
    let j = aberta_na_sessao(cx, &publicador);
    let pedidos = Arc::new(Mutex::new(Vec::new()));
    let anotados = pedidos.clone();
    let entidade = j.tela.clone();
    cx.update(|cx| {
        cx.subscribe(&entidade, move |_, p: &PedidoDoCaixa, _| {
            anotados.lock().unwrap().push(p.clone())
        })
        .detach();
    });
    com(cx, &j, |_, _, cx| {
        cx.emit(PedidoDoCaixa::AbrirSessao("g1".into()))
    });
    assert_eq!(
        *pedidos.lock().unwrap(),
        vec![PedidoDoCaixa::AbrirSessao("g1".into())]
    );
    assert_eq!(
        com(cx, &j, |t, _, _| t.sessao_escolhida().map(str::to_string)),
        Some("g1".into())
    );
}
