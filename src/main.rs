mod bus;
mod config;
mod control;
mod dot;
mod model;
mod monitor;
mod probe;
mod stats;
mod tray;

use config::{CheckKind, ControlKind, Node};
use model::ServiceView;
use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;

const USAGE: &str = "\
livedot: a red, yellow or green dot for each service you care about

usage:
  livedot daemon [--config PATH]   probe the configured services and serve their state
  livedot tray                     show the dot in the system tray
  livedot status [--json]          print the state the daemon holds
  livedot start ID                 start a service or wrapper, for example shop or shop/web
  livedot stop ID
  livedot restart ID
  livedot prompt ID [--button NAME] [TEXT...]
                                   hand text to a prompt service or one of its buttons;
                                   reads standard input when no TEXT is given
  livedot open ID                  open the service's url in the default browser
  livedot dots [--json]            list every top-level service by name and whether it is in the bar
  livedot dots --pick              tick the services to show in a checklist window
  livedot show ID                  give a top-level service its place in the bar again
  livedot hide ID                  take a top-level service out of the bar; it is still checked
  livedot dotsize [PERCENT]        set the dot size as a percent of the default, 50 to 200, or print it
  livedot healthy hide|show        draw no dot for a green service in the per-service layout, or draw it
  livedot item ID ITEM ACTION      run one item action on an item the service reports now
  livedot stats [--low-gb N] [--state PATH]
                                   print free disk space, memory, load and swap as the JSON a
                                   command check reads
  livedot stats show ITEM [--state PATH]
                                   pick which of them the label shows
  livedot check [--config PATH]    validate a config and print what it declares
  livedot --version";

fn config_path(args: &[String]) -> Result<PathBuf, String> {
    match args.iter().position(|a| a == "--config") {
        Some(index) => args
            .get(index + 1)
            .map(PathBuf::from)
            .ok_or_else(|| "--config needs a path".to_string()),
        None => Ok(config::default_path()),
    }
}

fn describe(node: &Node, depth: usize) {
    let check = match node.check.as_ref().map(|c| &c.kind) {
        Some(CheckKind::Http { url, .. }) => format!("http {url}"),
        Some(CheckKind::Unit { unit, .. }) => format!("unit {unit}"),
        Some(CheckKind::Process { pattern }) => format!("process {pattern:?}"),
        Some(CheckKind::Command { command: config::CommandSpec::Shell(line) }) => format!("command {line:?}"),
        Some(CheckKind::Command { command: config::CommandSpec::Argv(argv) }) => format!("command {}", argv.join(" ")),
        None if node.prompt.is_some() => "prompt".to_string(),
        None => "wrapper".to_string(),
    };
    let control = match node.control.as_ref().map(|c| &c.kind) {
        Some(ControlKind::Unit { unit, .. }) => format!("control unit {unit}"),
        Some(ControlKind::Commands { .. }) => "control commands".to_string(),
        None => match &node.prompt {
            Some(prompt) if prompt.buttons.is_empty() => "runs its command with the text on standard input".to_string(),
            Some(prompt) => format!(
                "runs its command with the text on standard input; buttons {}",
                prompt.buttons.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(",")
            ),
            None if node.children.is_empty() => "no control".to_string(),
            None => "control via children".to_string(),
        },
    };
    let actions = control::available(node);
    let actions = if actions.is_empty() { "no actions".to_string() } else { actions.join(",") };
    let off = format!(
        "{}{}",
        if node.enabled { "" } else { "  [disabled]" },
        if node.rollup { "" } else { "  [no rollup]" }
    );
    let off = format!("{off}{}", if node.one_click { "  [one click]" } else { "" });
    let icon = match &node.icon {
        Some(path) => match dot::Logo::load(path, &[tray::MENU_LOGO]) {
            Ok(_) => format!("  icon {}", path.display()),
            Err(error) => format!("  [icon falls back to a plain dot: {error}]"),
        },
        None => String::new(),
    };
    let url = node.url.as_ref().map(|url| format!("  url {url}")).unwrap_or_default();
    let items = if node.item_actions.is_empty() {
        String::new()
    } else {
        format!("  item actions {}", node.item_actions.iter().map(|b| b.name.as_str()).collect::<Vec<_>>().join(","))
    };
    println!("{}{}  {check}  {control}  ({actions}){icon}{url}{items}{off}", "  ".repeat(depth), node.id);
    for child in &node.children {
        describe(child, depth + 1);
    }
}

fn print_view(view: &ServiceView, depth: usize, color: bool) {
    let dot = if color {
        let [r, g, b] = view.color.rgb();
        format!("\x1b[38;2;{r};{g};{b}m●\x1b[0m")
    } else {
        "●".to_string()
    };
    let busy = if view.busy { "  [busy]" } else { "" };
    println!("{}{dot} {}  {}  {}{busy}", "  ".repeat(depth), view.name, view.status.word(), view.detail);
    if let Some(result) = &view.last_action {
        if !result.ok {
            println!("{}  last {} failed: {}", "  ".repeat(depth), result.action, result.message);
        }
    }
    for item in &view.items {
        let detail = item.detail.as_deref().map(|d| format!("  {d}")).unwrap_or_default();
        let actions = item.actions.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(",");
        println!("{}  · {}  {}{detail}  [{actions}]", "  ".repeat(depth), item.id, item.label);
    }
    for child in &view.children {
        print_view(child, depth + 1, color);
    }
}

fn run(args: &[String]) -> Result<ExitCode, (u8, String)> {
    let command = args.first().map(String::as_str).unwrap_or("--help");
    let rest = &args[args.len().min(1)..];
    match command {
        "daemon" => {
            let path = config_path(rest).map_err(|e| (2, e))?;
            let config = config::load(&path).map_err(|e| (2, e.to_string()))?;
            let probes_disabled = std::env::var("LIVEDOT_DISABLE_PROBES").is_ok_and(|v| v == "1");
            let (sender, receiver) = mpsc::channel();
            let shared = monitor::Shared::new(config, path.clone(), probes_disabled, sender);
            eprintln!(
                "livedot: daemon {} started from {}, {} top-level services{}",
                env!("CARGO_PKG_VERSION"),
                path.display(),
                shared.config.services.len(),
                if probes_disabled { ", probes disabled by LIVEDOT_DISABLE_PROBES" } else { "" }
            );
            bus::serve(shared, receiver).map_err(|e| (1, e))?;
            Ok(ExitCode::SUCCESS)
        }
        "tray" => {
            tray::run().map_err(|e| (1, e))?;
            Ok(ExitCode::SUCCESS)
        }
        "status" => {
            let proxy = bus::client().map_err(|e| (3, e))?;
            if rest.iter().any(|a| a == "--json") {
                let json = proxy.get_state().map_err(|e| (3, format!("livedot daemon is not answering: {e}")))?;
                println!("{json}");
            } else {
                let snapshot = bus::fetch(&proxy).map_err(|e| (3, e))?;
                let color = std::io::stdout().is_terminal();
                for view in &snapshot.services {
                    print_view(view, 0, color);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        "start" | "stop" | "restart" => {
            let id = rest.first().ok_or_else(|| (2, format!("{command} needs a service id")))?;
            let proxy = bus::client().map_err(|e| (3, e))?;
            proxy.control(id, command).map_err(|e| (1, format!("{command} {id} refused: {e}")))?;
            println!("{command} requested for {id}");
            Ok(ExitCode::SUCCESS)
        }
        "prompt" => {
            let id = rest.first().ok_or_else(|| (2, "prompt needs a service id".to_string()))?;
            let mut words = &rest[1..];
            let mut button = None;
            if words.first().is_some_and(|w| w == "--button") {
                button = Some(words.get(1).ok_or_else(|| (2, "--button needs a name".to_string()))?.clone());
                words = &words[2..];
            }
            let text = if !words.is_empty() {
                words.join(" ")
            } else {
                let mut text = String::new();
                std::io::stdin().read_to_string(&mut text).map_err(|e| (2, format!("cannot read standard input: {e}")))?;
                text
            };
            let proxy = bus::client().map_err(|e| (3, e))?;
            match &button {
                Some(button) => proxy.prompt_button(id, button, &text),
                None => proxy.prompt(id, &text),
            }
            .map_err(|e| (1, format!("prompt {id} refused: {e}")))?;
            println!("prompt accepted by {id}");
            Ok(ExitCode::SUCCESS)
        }
        "open" => {
            let id = rest.first().ok_or_else(|| (2, "open needs a service id".to_string()))?;
            let proxy = bus::client().map_err(|e| (3, e))?;
            let snapshot = bus::fetch(&proxy).map_err(|e| (3, e))?;
            let view = snapshot
                .flat()
                .into_iter()
                .find(|view| &view.id == id)
                .ok_or_else(|| (1, format!("unknown service {id:?}")))?;
            let url = view.url.as_ref().ok_or_else(|| (1, format!("service {id:?} has no url")))?;
            tray::open(url).map_err(|e| (1, e))?;
            println!("opened {url}");
            Ok(ExitCode::SUCCESS)
        }
        "show" | "hide" => {
            let id = rest.first().ok_or_else(|| (2, format!("{command} needs a service id")))?;
            let proxy = bus::client().map_err(|e| (3, e))?;
            proxy.set_shown(id, command == "show").map_err(|e| (1, format!("{command} {id} refused: {e}")))?;
            println!("{id} {}", if command == "show" { "shown" } else { "hidden" });
            Ok(ExitCode::SUCCESS)
        }
        "dots" => {
            let proxy = bus::client().map_err(|e| (3, e))?;
            if rest.iter().any(|a| a == "--pick") {
                tray::pick_dots(&proxy).map_err(|e| (1, e))?;
                return Ok(ExitCode::SUCCESS);
            }
            let snapshot = bus::fetch(&proxy).map_err(|e| (3, e))?;
            let mut views: Vec<&ServiceView> = snapshot.services.iter().collect();
            views.sort_by_key(|view| view.name.to_lowercase());
            if rest.iter().any(|a| a == "--json") {
                let rows: Vec<serde_json::Value> = views
                    .iter()
                    .map(|view| {
                        serde_json::json!({
                            "id": view.id,
                            "name": view.name,
                            "shown": !view.hidden,
                            "status": view.status.word(),
                            "color": view.color,
                        })
                    })
                    .collect();
                println!("{}", serde_json::Value::Array(rows));
            } else {
                let color = std::io::stdout().is_terminal();
                let width = views.iter().map(|view| view.name.chars().count()).max().unwrap_or(0);
                for view in views {
                    let dot = if color {
                        let [r, g, b] = view.color.rgb();
                        format!("\x1b[38;2;{r};{g};{b}m●\x1b[0m")
                    } else {
                        "●".to_string()
                    };
                    let pad = " ".repeat(width - view.name.chars().count());
                    println!("{dot} {}{pad}  {}  {}", view.name, if view.hidden { "hidden" } else { "shown" }, view.status.word());
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        "dotsize" => {
            let proxy = bus::client().map_err(|e| (3, e))?;
            match rest.first() {
                Some(text) => {
                    let percent: u32 =
                        text.trim_end_matches('%').parse().map_err(|_| (2, format!("dotsize needs a number, got {text:?}")))?;
                    proxy.set_dot_size(percent).map_err(|e| (1, format!("dotsize {percent} refused: {e}")))?;
                    println!("dot size {percent}%");
                }
                None => {
                    let snapshot = bus::fetch(&proxy).map_err(|e| (3, e))?;
                    println!("dot size {}%", snapshot.dot_size);
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        "healthy" => {
            let hide = match rest.first().map(String::as_str) {
                Some("hide") => true,
                Some("show") => false,
                _ => return Err((2, "healthy needs hide or show".to_string())),
            };
            let proxy = bus::client().map_err(|e| (3, e))?;
            proxy.set_hide_healthy(hide).map_err(|e| (1, format!("healthy refused: {e}")))?;
            println!("{}", if hide { "healthy dots hidden" } else { "healthy dots shown" });
            Ok(ExitCode::SUCCESS)
        }
        "item" => {
            let [id, item, action] = [0, 1, 2].map(|index| rest.get(index));
            let (Some(id), Some(item), Some(action)) = (id, item, action) else {
                return Err((2, "item needs a service id, an item id and an action".to_string()));
            };
            let proxy = bus::client().map_err(|e| (3, e))?;
            proxy.item_action(id, item, action).map_err(|e| (1, format!("item {action} {item} on {id} refused: {e}")))?;
            println!("{action} accepted for {item} on {id}");
            Ok(ExitCode::SUCCESS)
        }
        "stats" => stats::run(rest),
        "check" => {
            let path = config_path(rest).map_err(|e| (2, e))?;
            let config = config::load(&path).map_err(|e| (2, e.to_string()))?;
            println!("{}: ok, layout {:?}", path.display(), config.layout);
            if !config.shell_panel {
                println!("shell_panel: false");
            }
            if config.dot_size != 100 {
                println!("dot_size: {}", config.dot_size);
            }
            if config.hide_healthy {
                println!("hide_healthy: true");
            }
            for node in &config.services {
                describe(node, 0);
            }
            Ok(ExitCode::SUCCESS)
        }
        "--version" | "-V" => {
            println!("livedot {}", env!("CARGO_PKG_VERSION"));
            Ok(ExitCode::SUCCESS)
        }
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        }
        other => Err((2, format!("unknown command {other:?}\n\n{USAGE}"))),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err((code, message)) => {
            eprintln!("livedot: {message}");
            ExitCode::from(code)
        }
    }
}
