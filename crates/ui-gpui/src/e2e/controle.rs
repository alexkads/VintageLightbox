//! Commands go through the production dispatcher against the real app render,
//! backed by the same fake services as the other UI scenarios.
use super::{abrir_o_app, Cenario, Estudio};
use crate::controle::execute;
use gpui_kit::{AppContext as _, TestAppContext, VisualTestContext};
use serde_json::Value;
use vlb_controle::Command;

fn command(e: &Estudio, cx: &mut TestAppContext, command: Command) -> anyhow::Result<Value> {
    let result = cx.update(|cx| execute(command, e.raiz.into(), cx));
    cx.run_until_parked();
    result
}

#[gpui_kit::test]
fn controle_inspeciona_a_janela_e_despacha_atalhos_do_app(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    e.entrar_na_conta(cx);
    cx.update_window(e.raiz.into(), |_, w, _| w.enable_local_control())
        .unwrap();
    cx.run_until_parked();
    let status = command(&e, cx, Command::Status).unwrap();
    assert_eq!(status["windows"].as_array().unwrap().len(), 1);
    let before = e.app(cx, |app, _, _| app.menu_lateral_aberto());
    command(
        &e,
        cx,
        Command::Key {
            key: "ctrl-b".into(),
            window: None,
        },
    )
    .unwrap();
    assert_ne!(e.app(cx, |app, _, _| app.menu_lateral_aberto()), before);
    let snapshot = command(
        &e,
        cx,
        Command::Describe {
            window: None,
            query: None,
            full: false,
        },
    )
    .unwrap();
    assert!(snapshot["total"].as_u64().unwrap() > 1, "{snapshot}");
    assert!(snapshot["controls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["role"] == "Button"));
    assert!(snapshot["controls"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["id"].is_string()));
    let bounds = VisualTestContext::from_window(e.raiz.into(), cx)
        .debug_bounds("botao-do-menu")
        .unwrap();
    let center = bounds.center();
    command(
        &e,
        cx,
        Command::Click {
            x: center.x.into(),
            y: center.y.into(),
            right: false,
            window: None,
        },
    )
    .unwrap();
    assert_eq!(e.app(cx, |app, _, _| app.menu_lateral_aberto()), before);
    assert!(command(
        &e,
        cx,
        Command::Node {
            id: u64::MAX,
            action: "Click".into(),
            value: None,
            window: None
        }
    )
    .is_err());
    assert!(command(
        &e,
        cx,
        Command::Click {
            x: -1.,
            y: 0.,
            right: false,
            window: None
        }
    )
    .is_err());
    assert!(command(
        &e,
        cx,
        Command::Describe {
            window: Some(u64::MAX),
            query: None,
            full: false
        }
    )
    .is_err());
}

#[gpui_kit::test]
fn controle_digita_no_campo_focado_e_inspeciona_seu_valor(cx: &mut TestAppContext) {
    let e = abrir_o_app(cx, Cenario::default());
    cx.update_window(e.raiz.into(), |_, w, _| w.enable_local_control())
        .unwrap();
    cx.run_until_parked();
    command(
        &e,
        cx,
        Command::Key {
            key: "cmd-a".into(),
            window: None,
        },
    )
    .unwrap();
    command(
        &e,
        cx,
        Command::Text {
            text: "joão@example.test".into(),
            window: None,
        },
    )
    .unwrap();
    let snapshot = command(
        &e,
        cx,
        Command::Describe {
            window: None,
            query: Some("joão@example.test".into()),
            full: false,
        },
    )
    .unwrap();
    assert!(snapshot["matched"].as_u64().unwrap() > 0, "{snapshot}");
    assert!(snapshot["controls"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["value"] == "joão@example.test"));
    let input = snapshot["controls"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["value"] == "joão@example.test")
        .unwrap();
    assert_eq!(snapshot["focus"], input["id"], "{snapshot}");
    let id = input["id"].as_str().unwrap().parse().unwrap();
    command(
        &e,
        cx,
        Command::Node {
            id,
            action: "SetValue".into(),
            value: Some("outra@example.test".into()),
            window: None,
        },
    )
    .unwrap();
    let changed = command(
        &e,
        cx,
        Command::Describe {
            window: None,
            query: Some("outra@example.test".into()),
            full: false,
        },
    )
    .unwrap();
    assert!(changed["matched"].as_u64().unwrap() > 0, "{changed}");
}
