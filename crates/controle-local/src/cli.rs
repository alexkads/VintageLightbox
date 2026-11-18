use crate::{call, error, Command};
use serde_json::Value;
use std::{path::PathBuf, time::Duration};

const HELP: &str = "vlb [--dir PATH] [--timeout MS] [--window ID] COMMAND\n\
Commands (JSON output):\n\
  status                         Instance and windows\n\
  inspect [FILTER]               Compact controls, focus and available actions\n\
  tree                           Full accessibility tree for diagnostics\n\
  screenshot PATH.png            Capture only the selected app window\n\
  key ctrl-a                     Dispatch a GPUI keystroke\n\
  text 'some text'                Insert text into the focused input\n\
  click X Y [right]              Coordinates in window content points\n\
  scroll X Y DY                  Wheel delta in points (negative = down)\n\
  drag X Y TO_X TO_Y             Drag with left mouse button\n\
  action NAME [JSON]             Invoke an available app action\n\
  node ID ACTION [VALUE]         Accessibility action, e.g. Focus, Click, SetValue\n\
  request JSON                   Send a typed protocol command\n\
Start the app with VLB_CONTROLE=PATH; use the same directory here.\n\
Exit codes: 0 success, 1 app/transport failure, 2 invalid arguments.\n\
Commands acknowledge dispatch; inspect again to confirm asynchronous results.";

struct Options {
    dir: PathBuf,
    timeout: Duration,
    command: Command,
}

fn parse(mut args: Vec<String>) -> Result<Options, String> {
    let mut dir = std::env::var_os("VLB_CONTROLE").map(PathBuf::from);
    let mut timeout = Duration::from_secs(10);
    let mut window = None;
    while args.first().is_some_and(|a| a.starts_with("--")) {
        if args.len() < 2 {
            return Err("global option needs a value".into());
        }
        let flag = args.remove(0);
        let value = args.remove(0);
        match flag.as_str() {
            "--dir" => dir = Some(PathBuf::from(value)),
            "--timeout" => {
                let ms: u64 = value.parse().map_err(|_| "invalid timeout")?;
                if !(1..=60_000).contains(&ms) {
                    return Err("timeout must be 1..60000 ms".into());
                }
                timeout = Duration::from_millis(ms);
            }
            "--window" => window = Some(value.parse().map_err(|_| "invalid window ID")?),
            _ => return Err(format!("unknown option: {flag}")),
        }
    }
    let verb = args.first().ok_or("missing command; use --help")?.as_str();
    let rest = &args[1..];
    let number = |i: usize| -> Result<f32, String> {
        let n: f32 = rest
            .get(i)
            .ok_or("missing coordinate")?
            .parse()
            .map_err(|_| "invalid coordinate")?;
        if !n.is_finite() {
            return Err("coordinate must be finite".into());
        }
        Ok(n)
    };
    let command = match (verb, rest.len()) {
        ("status", 0) => Command::Status,
        ("inspect" | "describe", 0 | 1) => Command::Describe {
            window,
            query: rest.first().cloned(),
            full: false,
        },
        ("tree", 0) => Command::Describe {
            window,
            query: None,
            full: true,
        },
        ("screenshot", 1) => Command::Screenshot {
            path: std::path::absolute(&rest[0]).map_err(|e| e.to_string())?,
            window,
        },
        ("key", 1) => Command::Key {
            key: rest[0].clone(),
            window,
        },
        ("text", 1) => Command::Text {
            text: rest[0].clone(),
            window,
        },
        ("click", 2 | 3) => {
            if rest.len() == 3 && rest[2] != "right" {
                return Err("expected 'right'".into());
            }
            Command::Click {
                x: number(0)?,
                y: number(1)?,
                right: rest.len() == 3,
                window,
            }
        }
        ("scroll", 3) => Command::Scroll {
            x: number(0)?,
            y: number(1)?,
            dy: number(2)?,
            window,
        },
        ("drag", 4) => Command::Drag {
            x: number(0)?,
            y: number(1)?,
            to_x: number(2)?,
            to_y: number(3)?,
            window,
        },
        ("action", 1 | 2) => Command::Action {
            name: rest[0].clone(),
            args: rest
                .get(1)
                .map(|s| serde_json::from_str(s))
                .transpose()
                .map_err(|e| e.to_string())?,
            window,
        },
        ("node", 2 | 3) => Command::Node {
            id: rest[0].parse().map_err(|_| "invalid node ID")?,
            action: rest[1].clone(),
            value: rest.get(2).cloned(),
            window,
        },
        ("request", 1) => serde_json::from_str(&rest[0]).map_err(|e| e.to_string())?,
        _ => {
            return Err(format!(
                "unknown command or incorrect arguments: {verb}; use --help"
            ))
        }
    };
    let dir = dir
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or("set VLB_CONTROLE or pass --dir PATH")?;
    Ok(Options {
        dir,
        timeout,
        command,
    })
}

pub fn run(args: Vec<String>) -> i32 {
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!("{HELP}");
        return 0;
    }
    let options = match parse(args) {
        Ok(options) => options,
        Err(e) => {
            println!("{}", error(e));
            return 2;
        }
    };
    let response = call(&options.dir, options.command, options.timeout).unwrap_or_else(error);
    let code = if response.get("ok") == Some(&Value::Bool(true)) {
        0
    } else {
        1
    };
    println!("{response}");
    code
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        [
            vec!["--dir".into(), "/tmp/vlb-test".into()],
            values.iter().map(|s| s.to_string()).collect(),
        ]
        .concat()
    }
    #[test]
    fn parses_window_and_coordinates_without_losing_negative_values() {
        let result = parse(args(&["--window", "42", "scroll", "20", "30", "-160"])).unwrap();
        assert!(matches!(
            result.command,
            Command::Scroll {
                window: Some(42),
                dy: -160.,
                ..
            }
        ));
    }
    #[test]
    fn rejects_ambiguous_or_unbounded_arguments() {
        for input in [
            vec!["click", "NaN", "2"],
            vec!["--timeout", "0", "status"],
            vec!["status", "extra"],
            vec!["node", "12", "Click", "a", "b"],
        ] {
            assert!(parse(args(&input)).is_err(), "{input:?}");
        }
    }
}
