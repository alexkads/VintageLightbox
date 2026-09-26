//! 📅 A agenda no desktop — cada gesto do `/dashboard/agendamentos` do site,
//! com o app inteiro montado e a tela clicada onde o dedo clica.
//!
//! Os ensaios do cenário caem **hoje, no relógio do estúdio**: a agenda abre
//! no dia de hoje, como a do site, e o teste precisa que eles estejam no mês
//! visível e na lista "Agendamentos de hoje".

use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use gpui::{Context, TestAppContext, VisualTestContext, Window};
use serde_json::{json, Value};

use super::chatbot::{clicar, desenhado, passo, toasts};
use super::{abrir_o_app, Cenario, Estudio};
use crate::agenda::modelo::{self, Visao};
use crate::agenda::tela::Modo;
use crate::agenda::Agenda;
use crate::app::Tela;
use crate::chatbot::modelo::fuso_do_estudio;
use crate::pos_venda::porta::PedidoJson;
use crate::tempo_real::{Aviso, EstadoDaConexao, Sinal};

fn hoje() -> NaiveDate {
    modelo::hoje(Utc::now())
}

/// `hh:mm` de hoje (mais `dias`) no estúdio, em UTC.
fn as_(dias: i64, hora: u32, minuto: u32) -> DateTime<Utc> {
    let dia = hoje() + Duration::days(dias);
    fuso_do_estudio()
        .from_local_datetime(&dia.and_hms_opt(hora, minuto, 0).unwrap())
        .single()
        .unwrap()
        .with_timezone(&Utc)
}

fn iso(instante: DateTime<Utc>) -> String {
    instante.to_rfc3339()
}

fn ensaio(id: &str, nome: &str, status: &str, inicio: DateTime<Utc>, minutos: i64) -> Value {
    json!({
        "id": id, "user_id": "u", "status": status,
        "start_time": iso(inicio), "end_time": iso(inicio + Duration::minutes(minutos)),
        "created_at": iso(inicio), "updated_at": iso(inicio),
        "studio_id": "s1", "estudio": "Gramado", "cidade": "Gramado",
        "nome_familia": nome, "whatsapp_phone": "5554999991234", "origem": "WhatsApp",
        "valor_pago": if id == "e1" { json!("250.00") } else { Value::Null },
        "quantidade_de_pessoas": 4, "tipo_ensaio": "FAMILIA", "notes": null,
        "hora_entrada": null, "hora_saida": null, "observacoes_atendimento": null
    })
}

/// Cinco ensaios hoje (três aparecem no quadradinho do mês, "+ 2 mais") e
/// um amanhã, que é o do aviso.
fn ensaios() -> Vec<Value> {
    vec![
        ensaio("e1", "Família Souza", "Confirmed", as_(0, 10, 0), 90),
        ensaio("e2", "Ana e Bia", "Cancelled", as_(0, 11, 0), 60),
        ensaio("e3", "Família Lima", "Pending", as_(0, 13, 0), 60),
        ensaio("e4", "Casal Prado", "Completed", as_(0, 15, 0), 60),
        ensaio("e5", "Família Reis", "Rescheduled", as_(0, 17, 0), 60),
        ensaio("e9", "Família Nova", "Pending", as_(1, 9, 0), 60),
    ]
}

fn preparar_o_site(e: &Estudio) {
    let s = &e.site;
    s.responder_json(
        "estudios",
        Ok(json!([
            {"id": "s1", "name": "Gramado", "is_active": true},
            {"id": "s2", "name": "Canela", "is_active": true},
            {"id": "s3", "name": "Fechado", "is_active": false}
        ])),
    );
    s.responder_json("agenda", Ok(Value::Array(ensaios())));
    s.responder_json("hoje", Ok(Value::Array(ensaios()[..5].to_vec())));
    s.responder_json(
        "indicadores",
        Ok(
            json!({"total": 8, "pending": 2, "confirmed": 5, "cancelled": 1, "rescheduled": 0,
                  "completed": 0, "ocupacao_hoje": 67, "total_da_semana": 3}),
        ),
    );
    s.responder_json("reagendado", Ok(json!({"id": "e1"})));
    s.responder_json("atendido", Ok(json!({"id": "e1"})));
    s.responder_json("excluido", Ok(Value::Null));
    // 🔑 A resposta real do descancelar é `{id, status}` — e não o
    // agendamento inteiro, que era o que o site esperava.
    s.responder_json("restaurado", Ok(json!({"id": "e2", "status": "CONFIRMED"})));
}

fn entrar(cx: &mut TestAppContext) -> Estudio {
    let e = abrir_o_app(cx, Cenario::default());
    preparar_o_site(&e);
    e.entrar_na_conta(cx);
    e
}

fn abrir_a_agenda(cx: &mut TestAppContext) -> Estudio {
    let e = entrar(cx);
    clicar(&e, cx, "menu-Agendamentos");
    e.esperar(cx);
    e
}

fn agenda<R>(
    e: &Estudio,
    cx: &mut TestAppContext,
    f: impl FnOnce(&mut Agenda, &mut Window, &mut Context<Agenda>) -> R,
) -> R {
    e.app(cx, |app, window, cx| {
        let agenda = app.agenda.clone();
        agenda.update(cx, |tela, cx| f(tela, window, cx))
    })
}

fn pedidos(e: &Estudio) -> Vec<PedidoJson> {
    e.site
        .pedidos_json()
        .into_iter()
        .filter(|p| p.caminho.starts_with("/bookings"))
        .collect()
}

fn caminhos(e: &Estudio) -> Vec<String> {
    pedidos(e)
        .into_iter()
        .map(|p| format!("{} {}", p.metodo, p.caminho))
        .collect()
}

fn leituras_da_agenda(e: &Estudio) -> usize {
    pedidos(e).iter().filter(|p| p.rotulo == "agenda").count()
}

fn escrever(
    e: &Estudio,
    cx: &mut TestAppContext,
    campo: fn(&Agenda) -> &gpui::Entity<gpui_component::input::InputState>,
    texto: &str,
) {
    agenda(e, cx, |t, w, cx| {
        let texto = texto.to_string();
        campo(t)
            .clone()
            .update(cx, |c, cx| c.set_value(texto, w, cx));
    });
}

fn periodo(visao: Visao, dia: NaiveDate) -> String {
    let (de, ate) = modelo::periodo_visivel(visao, dia);
    format!(
        "GET /bookings/agenda?from={}&to={}",
        de.format("%Y-%m-%d"),
        ate.format("%Y-%m-%d")
    )
}

// ── Abrir e navegar ────────────────────────────────────────────────────────

/// Escutar começa na entrada; ler, só na tela — como o chatbot.
#[gpui::test]
fn a_conta_entra_e_a_agenda_escuta_sem_ler(cx: &mut TestAppContext) {
    let e = entrar(cx);
    assert_eq!(e.escuta.abertas(), 2, "o chatbot e a agenda");
    assert_eq!(
        e.escuta.caminhos_da_fonte("agenda"),
        vec!["/bookings/agenda/eventos"]
    );
    assert!(pedidos(&e).is_empty(), "{:?}", caminhos(&e));
}

/// Pelo menu: os quatro pedidos do site, o mês de hoje com a folga de uma
/// semana, os estúdios só ativos, e o "+ 2 mais" que abre o dia.
#[gpui::test]
fn pelo_menu_le_os_quatro_e_mostra_o_mes(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Agenda));
    let h = hoje().format("%Y-%m-%d");
    assert_eq!(
        caminhos(&e),
        vec![
            "GET /bookings/studios".to_string(),
            periodo(Visao::Mes, hoje()),
            format!("GET /bookings/agenda?from={h}&to={h}"),
            "GET /bookings/stats".to_string(),
        ]
    );
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(t.visao, Visao::Mes);
        assert_eq!(t.estudios.len(), 2, "o estúdio desativado não aparece");
        assert_eq!(t.ensaios.len(), 6);
        assert_eq!(t.de_hoje.len(), 5);
        assert_eq!(t.indicadores.unwrap().taxa_de_confirmacao(), 63);
    });
    assert!(desenhado(&e, cx, "agenda-evento-e1"));
    let mais = format!("agenda-mais-{}", hoje().format("%Y-%m-%d"));
    clicar(&e, cx, &mais);
    agenda(&e, cx, |t, _w, _cx| assert_eq!(t.dia_aberto, Some(hoje())));
    clicar(&e, cx, "agenda-dia-evento-e5");
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(t.dia_aberto, None);
        assert_eq!(t.aberto.aberto().map(|a| a.id.as_str()), Some("e5"));
        assert_eq!(t.modo, Modo::Detalhes);
    });
}

/// Anterior, Próximo, Hoje e as seis visões pedem o período de cada uma; o
/// dia clicado no ano abre a visão de dia, e o nome do mês, a de mês.
#[gpui::test]
fn navegar_pede_o_periodo_de_cada_visao(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    clicar(&e, cx, "agenda-proximo");
    let proximo = modelo::navegar(Visao::Mes, hoje(), 1);
    assert!(caminhos(&e).contains(&periodo(Visao::Mes, proximo)));
    clicar(&e, cx, "agenda-hoje");
    agenda(&e, cx, |t, _w, _cx| assert_eq!(t.dia, hoje()));
    let n = leituras_da_agenda(&e);
    clicar(&e, cx, "agenda-hoje");
    assert_eq!(leituras_da_agenda(&e), n, "já em hoje, Hoje não relê");

    for (visao, alvo) in [
        (Visao::Semana, "agenda-visao-Semana"),
        (Visao::Dia, "agenda-visao-Dia"),
        (Visao::Agenda, "agenda-visao-Agenda"),
        (Visao::Estacao, "agenda-visao-Estação"),
        (Visao::Ano, "agenda-visao-Ano"),
    ] {
        clicar(&e, cx, alvo);
        let ultima = pedidos(&e)
            .into_iter()
            .rfind(|p| p.rotulo == "agenda")
            .map(|p| format!("{} {}", p.metodo, p.caminho));
        assert_eq!(ultima, Some(periodo(visao, hoje())), "{visao:?}");
    }
    clicar(&e, cx, "agenda-anterior");
    let ano_passado = modelo::navegar(Visao::Ano, hoje(), -1);
    agenda(&e, cx, |t, _w, _cx| assert_eq!(t.dia, ano_passado));
    clicar(&e, cx, "agenda-proximo");

    // Na visão de ano, o dia de hoje abre a visão de dia.
    let alvo = format!("agenda-dia-{}", hoje().format("%Y-%m-%d"));
    clicar(&e, cx, &alvo);
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!((t.visao, t.dia), (Visao::Dia, hoje()));
    });
    clicar(&e, cx, "agenda-visao-Ano");
    let mes = format!("agenda-mes-{}", hoje().format("%Y-%m"));
    clicar(&e, cx, &mes);
    agenda(&e, cx, |t, _w, _cx| assert_eq!(t.visao, Visao::Mes));
}

/// O seletor de estúdio recorta a agenda e o "hoje" — e não os indicadores.
#[gpui::test]
fn o_estudio_recorta_a_agenda_e_nao_os_indicadores(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    clicar(&e, cx, "agenda-seletor");
    agenda(&e, cx, |t, _w, _cx| assert!(t.escolhendo_estudio));
    clicar(&e, cx, "agenda-estudio-1");
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(t.estudio.as_deref(), Some("s1"));
        assert_eq!(t.nome_do_estudio_escolhido(), "Gramado");
        assert!(!t.escolhendo_estudio);
    });
    let c = caminhos(&e);
    let ultimas: Vec<&String> = c.iter().rev().take(4).collect();
    assert!(ultimas
        .iter()
        .any(|p| p.starts_with("GET /bookings/agenda?") && p.ends_with("&studio_id=s1")));
    assert_eq!(
        ultimas
            .iter()
            .filter(|p| p.contains("studio_id=s1"))
            .count(),
        2,
        "a agenda e o hoje: {ultimas:?}"
    );
    assert!(
        ultimas.contains(&&"GET /bookings/stats".to_string()),
        "os indicadores sem estúdio"
    );

    clicar(&e, cx, "agenda-seletor");
    clicar(&e, cx, "agenda-estudio-0");
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(t.estudio, None);
        assert_eq!(t.nome_do_estudio_escolhido(), "Todos os Estúdios");
    });
}

// ── O agendamento ──────────────────────────────────────────────────────────

/// Reagendar: os campos chegam preenchidos no fuso do estúdio; término antes
/// do início não sai; certo, vai em UTC e fecha com o toast do site. O `409`
/// fica no formulário com a frase do site.
#[gpui::test]
fn reagendar_valida_as_datas_e_manda_em_utc(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    clicar(&e, cx, "agenda-evento-e1");
    clicar(&e, cx, "detalhe-reagendar");
    agenda(&e, cx, |t, _w, cx| {
        assert_eq!(t.modo, Modo::Reagendar);
        assert_eq!(
            t.inicio.read(cx).value().as_ref(),
            modelo::para_o_campo(as_(0, 10, 0))
        );
        assert_eq!(
            t.fim.read(cx).value().as_ref(),
            modelo::para_o_campo(as_(0, 11, 30))
        );
    });

    let amanha = (hoje() + Duration::days(1)).format("%d/%m/%Y").to_string();
    escrever(&e, cx, |t| &t.inicio, &format!("{amanha} 15:00"));
    escrever(&e, cx, |t| &t.fim, &format!("{amanha} 14:00"));
    clicar(&e, cx, "reagendar-confirmar");
    assert!(
        !caminhos(&e).iter().any(|c| c.starts_with("PATCH")),
        "término antes do início"
    );
    agenda(&e, cx, |t, _w, cx| {
        assert_eq!(
            t.datas_do_reagendamento(cx),
            Err("A data de término deve ser após a data de início.")
        );
    });

    e.site.responder_json(
        "reagendado",
        Err("o site respondeu 409 Conflict: Horário sem vaga (1/1)".into()),
    );
    escrever(&e, cx, |t| &t.fim, &format!("{amanha} 16:30"));
    clicar(&e, cx, "reagendar-confirmar");
    e.esperar(cx);
    let conflito = "Já há ensaio nesse horário. Escolha outro para reagendar.";
    assert!(toasts(&e, cx).contains(&(conflito.into(), true)));
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(t.modo, Modo::Reagendar, "o formulário fica");
        assert_eq!(t.erro_do_formulario.as_deref(), Some(conflito));
    });

    e.site.responder_json("reagendado", Ok(json!({"id": "e1"})));
    clicar(&e, cx, "reagendar-confirmar");
    e.esperar(cx);
    let patch = pedidos(&e)
        .into_iter()
        .rfind(|p| p.metodo == "PATCH")
        .unwrap();
    assert_eq!(patch.caminho, "/bookings/e1/reschedule");
    assert_eq!(
        patch.corpo.unwrap(),
        json!({
            "start_time": modelo::do_campo(&format!("{amanha} 15:00")).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "end_time": modelo::do_campo(&format!("{amanha} 16:30")).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        })
    );
    assert!(toasts(&e, cx).contains(&("Agendamento reagendado.".into(), false)));
    agenda(&e, cx, |t, _w, _cx| {
        assert!(!t.aberto.esta_aberto(), "fecha")
    });
}

/// Registrar atendimento: o valor chega em reais, "dez" não passa, saída
/// antes da entrada não passa; certo, manda só o preenchido.
#[gpui::test]
fn registrar_atendimento_valida_e_manda_so_o_preenchido(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    clicar(&e, cx, "agenda-evento-e1");
    clicar(&e, cx, "detalhe-atendimento");
    agenda(&e, cx, |t, _w, cx| {
        assert_eq!(t.modo, Modo::Atendimento);
        assert_eq!(t.valor.read(cx).value().as_ref(), "250,00");
    });
    escrever(&e, cx, |t| &t.valor, "dez");
    clicar(&e, cx, "atendimento-confirmar");
    agenda(&e, cx, |t, _w, cx| {
        assert_eq!(
            t.dados_do_atendimento(cx),
            Err("Valor pago inválido.".into())
        )
    });

    let h = hoje().format("%d/%m/%Y").to_string();
    escrever(&e, cx, |t| &t.valor, "300,50");
    escrever(&e, cx, |t| &t.entrada, &format!("{h} 10:10"));
    escrever(&e, cx, |t| &t.saida, &format!("{h} 10:00"));
    clicar(&e, cx, "atendimento-confirmar");
    agenda(&e, cx, |t, _w, cx| {
        assert_eq!(
            t.dados_do_atendimento(cx),
            Err("A hora de saída deve ser após a hora de entrada.".into())
        )
    });
    assert!(!caminhos(&e).iter().any(|c| c.starts_with("PATCH")));

    escrever(&e, cx, |t| &t.saida, "");
    escrever(&e, cx, |t| &t.observacoes, "pagou no pix");
    clicar(&e, cx, "atendimento-confirmar");
    e.esperar(cx);
    let patch = pedidos(&e)
        .into_iter()
        .find(|p| p.metodo == "PATCH")
        .unwrap();
    assert_eq!(patch.caminho, "/bookings/e1/attendance");
    assert_eq!(
        patch.corpo.unwrap(),
        json!({
            "valor_pago": 300.5,
            "hora_entrada": as_(0, 10, 10).to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "observacoes_atendimento": "pagou no pix"
        }),
        "a saída vazia não vai"
    );
    assert!(toasts(&e, cx).contains(&("Atendimento registrado.".into(), false)));
}

/// Excluir pergunta antes; "Cancelar" volta aos detalhes sem mandar nada.
#[gpui::test]
fn excluir_pergunta_antes_e_usa_a_rota_do_painel(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    clicar(&e, cx, "agenda-evento-e1");
    clicar(&e, cx, "detalhe-excluir");
    agenda(&e, cx, |t, _w, _cx| assert_eq!(t.modo, Modo::Excluir));
    clicar(&e, cx, "excluir-cancelar");
    agenda(&e, cx, |t, _w, _cx| assert_eq!(t.modo, Modo::Detalhes));
    assert!(!caminhos(&e).iter().any(|c| c.starts_with("DELETE")));
    clicar(&e, cx, "detalhe-excluir");
    clicar(&e, cx, "excluir-confirmar");
    e.esperar(cx);
    assert!(caminhos(&e).contains(&"DELETE /bookings/agenda/e1".to_string()));
    assert!(toasts(&e, cx).contains(&("Agendamento excluído.".into(), false)));
    agenda(&e, cx, |t, _w, _cx| assert!(!t.aberto.esta_aberto()));
}

/// 🔧 **Descancelar pela lista de hoje** — e a resposta `{id, status}` é
/// sucesso. No site a mesma resposta era lida como o agendamento inteiro, e
/// um descancelamento que deu certo aparecia como erro.
#[gpui::test]
fn descancelar_pela_lista_de_hoje_aceita_a_resposta_curta(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    assert!(
        desenhado(&e, cx, "hoje-descancelar-1"),
        "só o cancelado tem o botão"
    );
    clicar(&e, cx, "hoje-descancelar-1");
    e.esperar(cx);
    assert!(caminhos(&e).contains(&"POST /bookings/e2/uncancel".to_string()));
    assert!(toasts(&e, cx).contains(&("Agendamento restaurado.".into(), false)));

    // O que não está cancelado não descancela, nem pelo gesto.
    agenda(&e, cx, |t, _w, cx| {
        let confirmado = t.de_hoje[0].clone();
        t.descancelar(&confirmado, cx);
    });
    assert_eq!(
        caminhos(&e)
            .iter()
            .filter(|c| c.ends_with("/uncancel"))
            .count(),
        1
    );
}

/// O Esc volta um passo: formulário → detalhes → fechado.
#[gpui::test]
fn o_esc_volta_um_passo(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    clicar(&e, cx, "agenda-evento-e1");
    clicar(&e, cx, "detalhe-reagendar");
    e.teclar(cx, "escape");
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(t.modo, Modo::Detalhes);
        assert!(t.aberto.esta_aberto());
    });
    e.teclar(cx, "escape");
    agenda(&e, cx, |t, _w, _cx| assert!(!t.aberto.esta_aberto()));
}

// ── O tempo real ───────────────────────────────────────────────────────────

fn criado(id: &str, quando: DateTime<Utc>, estudio: Option<&str>) -> Sinal {
    Sinal::Evento {
        fonte: "agenda",
        dados: json!({
            "tipo": "ensaio_criado", "ensaio_id": id, "quando": iso(quando),
            "estudio_id": estudio, "origem": "WhatsApp", "cliente": "Família Nova",
            "em": iso(Utc::now())
        }),
    }
}

/// 🔔 Agendamento novo com o operador em outra tela: toast com os textos do
/// site; com a janela atrás, aviso do sistema, e o clique abre a agenda e o
/// ensaio.
#[gpui::test]
fn agendamento_novo_avisa_e_o_clique_abre_o_ensaio(cx: &mut TestAppContext) {
    let e = entrar(cx);
    let quando = as_(1, 9, 0);
    let horario = quando
        .with_timezone(&fuso_do_estudio())
        .format("%d/%m, %H:%M")
        .to_string();

    e.escuta.mandar(criado("e9", quando, Some("s1")));
    e.esperar(cx);
    assert!(toasts(&e, cx).contains(&(
        format!("Novo agendamento pelo WhatsApp — Família Nova · {horario}"),
        false
    )));
    assert!(pedidos(&e).is_empty(), "escondida, não relê");

    VisualTestContext::from_window(e.raiz.into(), cx).deactivate_window();
    e.escuta.mandar(Sinal::Evento {
        fonte: "agenda",
        dados: json!({"tipo": "ensaio_cancelado", "ensaio_id": "e2", "quando": null,
                      "estudio_id": null, "origem": null, "cliente": "Ana e Bia"}),
    });
    e.escuta.mandar(criado("e9", quando, None));
    e.esperar(cx);
    assert_eq!(
        e.avisador.avisos(),
        vec![
            Aviso {
                titulo: "Agendamento cancelado".into(),
                corpo: "Ana e Bia".into(),
                destino: "agenda:e2".into()
            },
            Aviso {
                titulo: "Novo agendamento pelo WhatsApp".into(),
                corpo: format!("Família Nova · {horario}"),
                destino: "agenda:e9".into()
            },
        ]
    );
    // Só criado e cancelado avisam.
    e.escuta.mandar(Sinal::Evento {
        fonte: "agenda",
        dados: json!({"tipo": "ensaio_atualizado", "ensaio_id": "e1", "quando": iso(as_(0, 10, 0))}),
    });
    e.esperar(cx);
    assert_eq!(e.avisador.avisos().len(), 2);

    e.avisador.clicar_no_ultimo();
    e.esperar(cx);
    e.app(cx, |app, _w, _cx| assert_eq!(app.tela(), Tela::Agenda));
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(
            t.aberto.aberto().map(|a| a.id.as_str()),
            Some("e9"),
            "o ensaio do aviso abre quando a leitura chega"
        );
    });
}

/// Com a tela na frente, o evento relevante relê (400 ms, uma vez por
/// rajada); o de outro estúdio com o filtro ligado, não; e o relógio de
/// segurança é o do site.
#[gpui::test]
fn a_releitura_segue_a_relevancia_e_o_relogio(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    let mut n = leituras_da_agenda(&e);
    for _ in 0..4 {
        e.escuta.mandar(criado("e9", as_(1, 9, 0), Some("s1")));
    }
    e.esperar(cx);
    n += 1;
    assert_eq!(leituras_da_agenda(&e), n, "quatro eventos, uma releitura");

    clicar(&e, cx, "agenda-seletor");
    clicar(&e, cx, "agenda-estudio-1");
    e.esperar(cx);
    n += 1;
    e.escuta.mandar(criado("x", as_(1, 9, 0), Some("s2")));
    e.esperar(cx);
    assert_eq!(leituras_da_agenda(&e), n, "outro estúdio não relê");

    e.escuta.mandar(Sinal::Pronto { fonte: "agenda" });
    passo(cx);
    assert_eq!(leituras_da_agenda(&e), n, "o primeiro pronto é a abertura");
    e.escuta.mandar(Sinal::Pronto { fonte: "agenda" });
    passo(cx);
    passo(cx);
    n += 1;
    assert_eq!(leituras_da_agenda(&e), n, "reconectou: relê");

    e.escuta.mandar(Sinal::Conexao {
        fonte: "agenda",
        estado: EstadoDaConexao::Conectado,
    });
    passo(cx);
    agenda(&e, cx, |t, _w, _cx| {
        assert_eq!(t.conexao.rotulo(), "Tempo real")
    });
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(16));
    cx.run_until_parked();
    passo(cx);
    assert_eq!(leituras_da_agenda(&e), n, "conectado, 15 s não relê");
    e.escuta.mandar(Sinal::Conexao {
        fonte: "agenda",
        estado: EstadoDaConexao::Recusado,
    });
    passo(cx);
    n += 1;
    assert_eq!(
        leituras_da_agenda(&e),
        n,
        "caiu com 16 s desde a última leitura: a regra de 15 s relê na hora"
    );
    cx.executor()
        .advance_clock(std::time::Duration::from_secs(15));
    cx.run_until_parked();
    passo(cx);
    assert_eq!(leituras_da_agenda(&e), n + 1, "e de novo 15 s depois");
}

/// 🔔 O sino é um só: desligado no chatbot, cala o aviso da agenda.
#[gpui::test]
fn o_sino_do_chatbot_vale_para_a_agenda(cx: &mut TestAppContext) {
    let e = entrar(cx);
    let arquivo = e.app(cx, |app, _w, cx| {
        app.chatbot.read(cx).arquivo_de_preferencias()
    });
    agenda(&e, cx, |t, _w, _cx| t.usar_preferencias(arquivo));
    e.app(cx, |app, _w, cx| {
        app.chatbot.update(cx, |t, cx| t.alternar_avisos(cx));
    });
    VisualTestContext::from_window(e.raiz.into(), cx).deactivate_window();
    e.escuta.mandar(criado("e9", as_(1, 9, 0), None));
    e.esperar(cx);
    assert!(e.avisador.avisos().is_empty());
}

/// A agenda que não carrega dá lugar ao aviso do site; os indicadores
/// também, cada um no seu lugar.
#[gpui::test]
fn falha_de_leitura_vira_aviso_no_lugar(cx: &mut TestAppContext) {
    let e = entrar(cx);
    e.site
        .responder_json("agenda", Err("o site respondeu 400: período".into()));
    e.site.responder_json("indicadores", Err("sem rede".into()));
    clicar(&e, cx, "menu-Agendamentos");
    e.esperar(cx);
    agenda(&e, cx, |t, _w, _cx| {
        assert!(t.falhou_a_agenda);
        assert!(t.falhou_os_indicadores);
        assert_eq!(t.de_hoje.len(), 5, "o hoje continua valendo");
    });
}

/// Sair da conta fecha o fluxo da agenda também.
#[gpui::test]
fn sair_da_conta_fecha_a_agenda(cx: &mut TestAppContext) {
    let e = abrir_a_agenda(cx);
    e.app(cx, |app, _w, cx| app.sair_da_conta(cx));
    e.esperar(cx);
    assert_eq!(
        e.escuta.largadas.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "chatbot e agenda"
    );
    agenda(&e, cx, |t, _w, _cx| {
        assert!(t.ensaios.is_empty() && t.de_hoje.is_empty());
        assert!(!t.visivel);
    });
}
