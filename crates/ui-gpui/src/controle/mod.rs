//! The transport thread never touches GPUI. Requests are applied on the UI thread.
use gpui_kit::{
    accesskit, point, px, AnyWindowHandle, App, InputEvent, Keystroke, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent, Window,
};
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
use vlb_controle::{error, success, Command};

pub fn ligar(principal: AnyWindowHandle, cx: &mut App) {
    let Some(dir) = std::env::var_os("VLB_CONTROLE").filter(|s| !s.is_empty()) else {
        return;
    };
    let (server, requests) = match vlb_controle::start(Path::new(&dir)) {
        Ok(pair) => pair,
        Err(e) => {
            eprintln!("[Controle local] não foi possível iniciar: {e}");
            return;
        }
    };
    eprintln!(
        "[Controle local] habilitado em {}",
        Path::new(&dir).display()
    );
    cx.spawn(async move |cx| {
        let _server = server;
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(40))
                .await;
            cx.update(|cx| {
                for window in cx.windows() {
                    let _ = window.update(cx, |_, window, _| window.enable_local_control());
                }
                // Bound work per tick so an automation cannot starve rendering.
                for request in requests.try_iter().take(8) {
                    let response = if Instant::now() >= request.expires {
                        error("request expired before dispatch; no action was applied")
                    } else {
                        execute(request.command, principal, cx)
                            .map(success)
                            .unwrap_or_else(error)
                    };
                    let _ = request.reply.send(response);
                }
            });
        }
    })
    .detach();
}

pub(crate) fn execute(
    command: Command,
    principal: AnyWindowHandle,
    cx: &mut App,
) -> anyhow::Result<Value> {
    let windows = cx.windows();
    if matches!(command, Command::Status) {
        let entries = windows
            .into_iter()
            .filter_map(|handle| {
                handle
                    .update(cx, |_, window, _| {
                        json!({
                            "id": handle.window_id().as_u64(),
                            "principal": handle == principal,
                            "width": f32::from(window.viewport_size().width),
                            "height": f32::from(window.viewport_size().height),
                            "scale_factor": window.scale_factor(),
                            "active": window.is_window_active(),
                        })
                    })
                    .ok()
            })
            .collect::<Vec<_>>();
        return Ok(
            json!({"pid":std::process::id(), "version":env!("CARGO_PKG_VERSION"), "windows":entries}),
        );
    }
    let handle = match command.window() {
        Some(id) => windows
            .into_iter()
            .find(|w| w.window_id().as_u64() == id)
            .ok_or_else(|| anyhow::anyhow!("window {id} is not open; use status"))?,
        None => principal,
    };
    handle.update(cx, |_, window, cx| execute_in_window(command, window, cx))?
}

fn execute_in_window(command: Command, window: &mut Window, cx: &mut App) -> anyhow::Result<Value> {
    let result = match command {
        Command::Status => unreachable!(),
        Command::Describe { query, full, .. } => {
            // Force a fresh frame: new windows, a modal or a prior command may
            // have changed both the accessibility IDs and mouse hitboxes.
            window.draw(cx).clear(cx);
            let tree = window
                .debug_a11y_tree_json()
                .map(|s| serde_json::from_str::<Value>(&s))
                .transpose()?
                .ok_or_else(|| anyhow::anyhow!("accessibility frame unavailable"))?;
            let actions = window
                .available_actions(cx)
                .iter()
                .map(|a| a.name().to_string())
                .collect::<Vec<_>>();
            let mut result = if full {
                json!({"tree":tree})
            } else {
                compact_tree(&tree, query.as_deref())
            };
            result["window"] = json!(window.window_handle().window_id().as_u64());
            if !full {
                result["focus"] = json!(window.local_control_focus().map(|id| id.to_string()));
            }
            result["actions"] = json!(actions);
            return Ok(result);
        }
        Command::Screenshot { path, .. } => {
            anyhow::ensure!(
                path.is_absolute(),
                "screenshot path must be absolute on the app host"
            );
            anyhow::ensure!(
                path.extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("png")),
                "screenshot path must end in .png"
            );
            window.draw(cx).clear(cx);
            #[cfg(target_os = "macos")]
            crate::depuracao::fotografar(window, &path).map_err(anyhow::Error::msg)?;
            #[cfg(not(target_os = "macos"))]
            window.render_to_image()?.save(&path)?;
            return Ok(json!({"path": path}));
        }
        Command::Key { key, .. } => {
            let key = Keystroke::parse(&key).map_err(anyhow::Error::msg)?;
            json!({"handled": window.dispatch_keystroke(key, cx)})
        }
        Command::Text { text, .. } => {
            window.insert_local_text(&text, cx)?;
            json!({"inserted_chars": text.chars().count()})
        }
        Command::Click { x, y, right, .. } => {
            position(window, x, y)?;
            let button = if right {
                MouseButton::Right
            } else {
                MouseButton::Left
            };
            move_mouse(window, cx, x, y, None);
            mouse_down(window, cx, x, y, button);
            mouse_up(window, cx, x, y, button);
            json!({"dispatched":true})
        }
        Command::Scroll { x, y, dy, .. } => {
            position(window, x, y)?;
            anyhow::ensure!(
                dy.is_finite() && dy.abs() <= 100_000.,
                "invalid wheel delta"
            );
            move_mouse(window, cx, x, y, None);
            window.dispatch_event(
                ScrollWheelEvent {
                    position: point(px(x), px(y)),
                    delta: ScrollDelta::Pixels(point(px(0.), px(dy))),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            json!({"dispatched":true})
        }
        Command::Drag {
            x, y, to_x, to_y, ..
        } => {
            position(window, x, y)?;
            position(window, to_x, to_y)?;
            move_mouse(window, cx, x, y, None);
            mouse_down(window, cx, x, y, MouseButton::Left);
            // Intermediate moves trigger the same drag threshold/listeners as
            // native input. Draw between moves to update drop-target hitboxes.
            for step in 1..=12 {
                let t = step as f32 / 12.;
                move_mouse(
                    window,
                    cx,
                    x + (to_x - x) * t,
                    y + (to_y - y) * t,
                    Some(MouseButton::Left),
                );
                window.draw(cx).clear(cx);
            }
            mouse_up(window, cx, to_x, to_y, MouseButton::Left);
            json!({"dispatched":true})
        }
        Command::Action { name, args, .. } => {
            anyhow::ensure!(
                window
                    .available_actions(cx)
                    .iter()
                    .any(|a| a.name() == name),
                "action is not available at the current focus; use inspect"
            );
            let action = cx.build_action(&name, args)?;
            window.dispatch_action(action, cx);
            json!({"dispatched":true, "action":name})
        }
        Command::Node {
            id, action, value, ..
        } => {
            window.draw(cx).clear(cx);
            let numeric = action.eq_ignore_ascii_case("SetNumericValue");
            let action = if numeric {
                "SetValue".to_string()
            } else {
                action
            };
            let action = (0..=u8::MAX)
                .filter_map(accesskit::Action::n)
                .find(|a| format!("{a:?}").eq_ignore_ascii_case(&action))
                .ok_or_else(|| anyhow::anyhow!("unknown accessibility action"))?;
            let data = match (action, value) {
                (accesskit::Action::SetValue, Some(value)) if numeric => {
                    let number: f64 = value.parse()?;
                    anyhow::ensure!(number.is_finite(), "value must be finite");
                    Some(accesskit::ActionData::NumericValue(number))
                }
                (accesskit::Action::SetValue, Some(value)) => {
                    Some(accesskit::ActionData::Value(value.into()))
                }
                (accesskit::Action::SetValue, None) => anyhow::bail!("SetValue requires a value"),
                (_, Some(_)) => anyhow::bail!("value is only supported for SetValue"),
                (_, None) => None,
            };
            window.dispatch_local_accessibility_action(
                accesskit::ActionRequest {
                    action,
                    target_tree: accesskit::TreeId::ROOT,
                    target_node: accesskit::NodeId(id),
                    data,
                },
                cx,
            )?;
            json!({"dispatched":true, "node":id.to_string()})
        }
    };
    window.refresh();
    Ok(result)
}

fn compact_tree(tree: &Value, query: Option<&str>) -> Value {
    let nodes = tree["nodes"].as_object();
    let filter = query.unwrap_or("").to_lowercase();
    let scale = tree["frame"]["scale_factor"]
        .as_f64()
        .filter(|s| *s > 0.)
        .unwrap_or(1.);
    let controls: Vec<Value> = nodes
        .into_iter()
        .flat_map(|nodes| nodes.values())
        .filter_map(|node| {
            let aria = &node["aria"];
            if !filter.is_empty() && !aria.to_string().to_lowercase().contains(&filter) {
                return None;
            }
            let mut control = aria.clone();
            control["id"] = node["accesskit_id"].clone();
            if let Some(bounds) = control
                .as_object_mut()
                .and_then(|map| map.remove("bounds_physical"))
            {
                control["bounds"] = json!({
                    "x":bounds["x"].as_f64().unwrap_or(0.)/scale,
                    "y":bounds["y"].as_f64().unwrap_or(0.)/scale,
                    "width":bounds["width"].as_f64().unwrap_or(0.)/scale,
                    "height":bounds["height"].as_f64().unwrap_or(0.)/scale,
                });
            }
            Some(control)
        })
        .collect();
    let focus = tree["gpui_focus"]
        .as_str()
        .and_then(|id| nodes.and_then(|n| n.get(id)))
        .map(|node| node["accesskit_id"].clone());
    json!({"frame":tree["frame"],"focus":focus,"total":nodes.map_or(0,|n| n.len()),"matched":controls.len(),"controls":controls})
}

fn position(window: &Window, x: f32, y: f32) -> anyhow::Result<()> {
    let size = window.viewport_size();
    anyhow::ensure!(
        x.is_finite()
            && y.is_finite()
            && x >= 0.
            && y >= 0.
            && x < f32::from(size.width)
            && y < f32::from(size.height),
        "coordinates must be inside the window content (in points)"
    );
    Ok(())
}
fn move_mouse(w: &mut Window, cx: &mut App, x: f32, y: f32, pressed_button: Option<MouseButton>) {
    w.dispatch_event(
        MouseMoveEvent {
            position: point(px(x), px(y)),
            pressed_button,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}
fn mouse_down(w: &mut Window, cx: &mut App, x: f32, y: f32, button: MouseButton) {
    w.dispatch_event(
        MouseDownEvent {
            position: point(px(x), px(y)),
            button,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}
fn mouse_up(w: &mut Window, cx: &mut App, x: f32, y: f32, button: MouseButton) {
    w.dispatch_event(
        MouseUpEvent {
            position: point(px(x), px(y)),
            button,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}
