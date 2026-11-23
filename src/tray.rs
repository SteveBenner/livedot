use crate::bus::{self, LivedotProxyBlocking};
use crate::control::PROMPT;
use crate::dot;
use crate::model::{Color, ItemView, Layout, ServiceView, Snapshot};
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, mpsc};
use std::time::Duration;

const POLL: Duration = Duration::from_secs(5);
const ICON_SIZES: [u32; 4] = [22, 32, 48, 64];
const MENU_DOT: u32 = 16;
pub const MENU_LOGO: u32 = 24;

type Logos = HashMap<String, Option<Arc<dot::Logo>>>;

fn logo_for<'a>(logos: &'a Logos, view: &ServiceView) -> Option<&'a dot::Logo> {
    logos.get(view.icon.as_ref()?)?.as_deref()
}

fn load_logos(logos: &mut Logos, snapshot: &Snapshot) {
    let mut sizes = ICON_SIZES.to_vec();
    sizes.push(MENU_LOGO);
    for view in snapshot.flat() {
        let Some(path) = &view.icon else { continue };
        if logos.contains_key(path) {
            continue;
        }
        let logo = match dot::Logo::load(Path::new(path), &sizes) {
            Ok(logo) => Some(Arc::new(logo)),
            Err(error) => {
                eprintln!("livedot tray: {} shows a plain dot: {error}", view.id);
                None
            }
        };
        logos.insert(path.clone(), logo);
    }
}

pub struct LiveTray {
    root: Option<String>,
    snapshot: Option<Snapshot>,
    problem: Option<String>,
    logos: Logos,
    connection: zbus::blocking::Connection,
}

impl LiveTray {
    fn service(&self) -> Option<&ServiceView> {
        let root = self.root.as_ref()?;
        self.snapshot.as_ref()?.services.iter().find(|s| &s.id == root)
    }

    fn color(&self) -> Color {
        if self.problem.is_some() {
            return Color::Yellow;
        }
        match (&self.root, &self.snapshot) {
            (Some(_), _) => self.service().map(|s| s.color).unwrap_or(Color::Yellow),
            (None, Some(snapshot)) => snapshot.color,
            (None, None) => Color::Yellow,
        }
    }

    fn summary(&self) -> String {
        if let Some(problem) = &self.problem {
            return problem.clone();
        }
        match (&self.root, &self.snapshot) {
            (Some(root), _) => match self.service() {
                Some(service) => {
                    let first = service.items.first().map(|item| format!(" · next: {}", item.label)).unwrap_or_default();
                    format!("{} {} ({}){first}", service.name, service.status.word(), service.detail)
                }
                None => format!("{root} is not in the daemon config"),
            },
            (None, Some(snapshot)) => snapshot
                .services
                .iter()
                .map(|s| format!("{} {}", s.name, s.status.word()))
                .collect::<Vec<_>>()
                .join(", "),
            (None, None) => "waiting for the livedot daemon".into(),
        }
    }

    fn ask(&self, id: &str, name: &str, button: Option<(String, String)>) {
        let connection = self.connection.clone();
        let id = id.to_string();
        let name = name.to_string();
        std::thread::spawn(move || {
            let hint = match &button {
                Some((_, label)) => label.trim_end_matches('…').trim_end_matches("...").to_string() + ":",
                None => format!("Tell {name}:"),
            };
            let dialog = std::process::Command::new("zenity")
                .args(["--entry", "--width=560", "--title", &name, "--text", &hint])
                .stderr(std::process::Stdio::null())
                .output();
            let text = match dialog {
                Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).trim().to_string(),
                Ok(_) => return,
                Err(error) => {
                    eprintln!("livedot tray: cannot open the text box for {id} (zenity): {error}");
                    return;
                }
            };
            if text.is_empty() {
                return;
            }
            let outcome = LivedotProxyBlocking::new(&connection).and_then(|proxy| match &button {
                Some((button, _)) => proxy.prompt_button(&id, button, &text),
                None => proxy.prompt(&id, &text),
            });
            if let Err(error) = outcome {
                eprintln!("livedot tray: prompt {id} refused: {error}");
            }
        });
    }

    fn run_item(&self, id: &str, item: &str, action: &str) {
        let connection = self.connection.clone();
        let (id, item, action) = (id.to_string(), item.to_string(), action.to_string());
        std::thread::spawn(move || {
            let outcome =
                LivedotProxyBlocking::new(&connection).and_then(|proxy| proxy.item_action(&id, &item, &action));
            if let Err(error) = outcome {
                eprintln!("livedot tray: {action} on {item} of {id} refused: {error}");
            }
        });
    }

    fn dots(&self) {
        let connection = self.connection.clone();
        std::thread::spawn(move || {
            let outcome = LivedotProxyBlocking::new(&connection)
                .map_err(|e| format!("cannot build the daemon proxy: {e}"))
                .and_then(|proxy| pick_dots(&proxy));
            if let Err(error) = outcome {
                eprintln!("livedot tray: dots: {error}");
            }
        });
    }

    fn send(&self, id: &str, action: &str) {
        let connection = self.connection.clone();
        let id = id.to_string();
        let action = action.to_string();
        std::thread::spawn(move || {
            let outcome = LivedotProxyBlocking::new(&connection).and_then(|proxy| proxy.control(&id, &action));
            if let Err(error) = outcome {
                eprintln!("livedot tray: {action} {id} refused: {error}");
            }
        });
    }
}

pub fn open(url: &str) -> Result<(), String> {
    let mut child = std::process::Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run xdg-open for {url}: {e}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

fn open_logged(url: &str) {
    if let Err(error) = open(url) {
        eprintln!("livedot tray: {error}");
    }
}

fn dots_items() -> Vec<MenuItem<LiveTray>> {
    vec![
        MenuItem::Separator,
        StandardItem {
            label: "Dots…".into(),
            icon_name: "view-list-symbolic".into(),
            activate: Box::new(|tray: &mut LiveTray| tray.dots()),
            ..Default::default()
        }
        .into(),
    ]
}

pub fn pick_dots(proxy: &LivedotProxyBlocking<'_>) -> Result<(), String> {
    let snapshot = bus::fetch(proxy)?;
    let mut views: Vec<&ServiceView> = snapshot.services.iter().collect();
    views.sort_by_key(|view| view.name.to_lowercase());
    let mut command = std::process::Command::new("zenity");
    command
        .args(["--list", "--checklist", "--title", "livedot dots", "--text", "Tick the dots to show in the bar"])
        .args(["--width", "420", "--height", "640"])
        .args(["--column", "Show", "--column", "Dot", "--column", "Status", "--hide-column", "4"])
        .args(["--column", "Id", "--print-column", "4", "--separator", "\n"]);
    for view in &views {
        command.args([if view.hidden { "FALSE" } else { "TRUE" }, &view.name, view.status.word(), &view.id]);
    }
    let output = command
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("cannot open the checklist (zenity): {e}"))?;
    if !output.status.success() {
        return Ok(());
    }
    let ticked: std::collections::HashSet<String> =
        String::from_utf8_lossy(&output.stdout).lines().map(|line| line.trim().to_string()).collect();
    let mut refused = Vec::new();
    for view in views {
        let want = ticked.contains(&view.id);
        if want == !view.hidden {
            continue;
        }
        if let Err(error) = proxy.set_shown(&view.id, want) {
            refused.push(format!("{}: {error}", view.id));
        }
    }
    if refused.is_empty() { Ok(()) } else { Err(refused.join("; ")) }
}

fn note(text: String) -> MenuItem<LiveTray> {
    StandardItem { label: text, enabled: false, ..Default::default() }.into()
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn service_items(view: &ServiceView, logos: &Logos) -> Vec<MenuItem<LiveTray>> {
    let header = StandardItem {
        label: format!("{} · {} · {}", view.name, view.status.word(), view.detail),
        icon_data: status_icon(view, logos),
        ..Default::default()
    };
    let mut items: Vec<MenuItem<LiveTray>> = vec![header.into()];
    if let Some(result) = &view.last_action {
        if !result.ok {
            items.push(note(format!("last {} failed: {}", result.action, result.message)));
        }
    }
    if view.busy {
        items.push(note("action in progress".into()));
    }
    if let Some(url) = &view.url {
        let url = url.clone();
        items.push(
            StandardItem {
                label: "Open in browser".into(),
                icon_name: "web-browser-symbolic".into(),
                activate: Box::new(move |_tray: &mut LiveTray| open_logged(&url)),
                ..Default::default()
            }
            .into(),
        );
    }
    for action in &view.actions {
        let id = view.id.clone();
        let verb = action.clone();
        if action == PROMPT {
            let name = view.name.clone();
            items.push(
                StandardItem {
                    label: "Ask…".into(),
                    icon_name: "document-edit-symbolic".into(),
                    enabled: !view.busy,
                    activate: Box::new(move |tray: &mut LiveTray| tray.ask(&id, &name, None)),
                    ..Default::default()
                }
                .into(),
            );
            for button in &view.buttons {
                let id = view.id.clone();
                let name = view.name.clone();
                let pair = (button.name.clone(), button.label.clone());
                items.push(
                    StandardItem {
                        label: button.label.clone(),
                        icon_name: button.icon.clone().unwrap_or_else(|| "document-edit-symbolic".into()),
                        enabled: !view.busy,
                        activate: Box::new(move |tray: &mut LiveTray| tray.ask(&id, &name, Some(pair.clone()))),
                        ..Default::default()
                    }
                    .into(),
                );
            }
            continue;
        }
        items.push(
            StandardItem {
                label: capitalize(action),
                icon_name: action_icon(action).into(),
                enabled: !view.busy,
                activate: Box::new(move |tray: &mut LiveTray| tray.send(&id, &verb)),
                ..Default::default()
            }
            .into(),
        );
    }
    if !view.items.is_empty() {
        items.push(MenuItem::Separator);
        items.extend(view.items.iter().map(|item| item_menu(view, item)));
    }
    if !view.children.is_empty() {
        items.push(MenuItem::Separator);
        items.extend(view.children.iter().map(|child| service_menu(child, logos)));
    }
    items
}

fn item_menu(view: &ServiceView, item: &ItemView) -> MenuItem<LiveTray> {
    if let Some(state) = item.checked {
        let first = item.actions.first().map(|action| action.name.clone());
        let (id, item_id) = (view.id.clone(), item.id.clone());
        return CheckmarkItem {
            label: item.label.clone(),
            checked: state,
            enabled: !view.busy && first.is_some(),
            activate: Box::new(move |tray: &mut LiveTray| {
                if let Some(name) = &first {
                    tray.run_item(&id, &item_id, name);
                }
            }),
            ..Default::default()
        }
        .into();
    }
    let mut entries: Vec<MenuItem<LiveTray>> = Vec::new();
    if let Some(detail) = &item.detail {
        entries.push(note(detail.clone()));
    }
    for action in &item.actions {
        let (id, item_id, name) = (view.id.clone(), item.id.clone(), action.name.clone());
        entries.push(
            StandardItem {
                label: action.label.clone(),
                icon_name: action.icon.clone().unwrap_or_default(),
                enabled: !view.busy,
                activate: Box::new(move |tray: &mut LiveTray| tray.run_item(&id, &item_id, &name)),
                ..Default::default()
            }
            .into(),
        );
    }
    SubMenu { label: item.label.clone(), submenu: entries, ..Default::default() }.into()
}

fn status_icon(view: &ServiceView, logos: &Logos) -> Vec<u8> {
    match logo_for(logos, view) {
        Some(logo) => logo.png(MENU_LOGO, view.color.rgb()),
        None => dot::png(MENU_DOT, view.color.rgb()),
    }
}

fn action_icon(action: &str) -> &'static str {
    match action {
        "start" => "media-playback-start-symbolic",
        "stop" => "media-playback-stop-symbolic",
        "restart" => "view-refresh-symbolic",
        _ => "",
    }
}

fn service_menu(view: &ServiceView, logos: &Logos) -> MenuItem<LiveTray> {
    SubMenu {
        label: format!("{} · {}", view.name, view.status.word()),
        icon_data: status_icon(view, logos),
        submenu: service_items(view, logos),
        ..Default::default()
    }
    .into()
}

impl ksni::Tray for LiveTray {
    fn id(&self) -> String {
        match &self.root {
            Some(root) => {
                let safe: String =
                    root.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
                format!("livedot-{safe}")
            }
            None => "livedot".into(),
        }
    }

    fn title(&self) -> String {
        match &self.root {
            Some(root) => format!("livedot: {root}"),
            None => "livedot".into(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        let Some(service) = self.service() else { return };
        if !service.actions.iter().any(|action| action == PROMPT) {
            if let Some(url) = &service.url {
                open_logged(url);
            } else if let Some((item, action)) =
                service.items.first().and_then(|item| item.actions.first().map(|action| (item, action)))
            {
                if !service.busy {
                    self.run_item(&service.id, &item.id, &action.name);
                }
            }
            return;
        }
        if service.busy {
            return;
        }
        let (id, name) = (service.id.clone(), service.name.clone());
        self.ask(&id, &name, None);
    }

    fn category(&self) -> ksni::Category {
        ksni::Category::SystemServices
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        let rgb = self.color().rgb();
        let logo = self.service().and_then(|service| logo_for(&self.logos, service));
        let unbadged = self.snapshot.as_ref().is_some_and(|snapshot| snapshot.hide_healthy)
            && self.service().is_some_and(|service| service.color == Color::Green);
        ICON_SIZES
            .iter()
            .map(|size| {
                let data = match logo {
                    Some(logo) if unbadged => logo.plain_argb(*size).unwrap_or_else(|| logo.argb(*size, rgb)),
                    Some(logo) => logo.argb(*size, rgb),
                    None => dot::argb(*size, rgb),
                };
                ksni::Icon { width: *size as i32, height: *size as i32, data }
            })
            .collect()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: self.title(), description: self.summary(), ..Default::default() }
    }

    fn label(&self) -> String {
        if self.problem.is_some() || self.root.is_none() {
            return String::new();
        }
        self.service().and_then(|service| service.label.clone()).unwrap_or_default()
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items = if let Some(problem) = &self.problem {
            vec![note(problem.clone())]
        } else {
            match (&self.root, &self.snapshot) {
                (Some(root), _) => match self.service() {
                    Some(service) => service_items(service, &self.logos),
                    None => vec![note(format!("{root} is not in the daemon config"))],
                },
                (None, Some(snapshot)) => snapshot
                    .services
                    .iter()
                    .filter(|service| !service.hidden)
                    .map(|service| service_menu(service, &self.logos))
                    .collect(),
                (None, None) => vec![note("waiting for the livedot daemon".into())],
            }
        };
        items.extend(dots_items());
        items
    }
}

fn keys(snapshot: &Snapshot) -> Vec<Option<String>> {
    match snapshot.layout {
        Layout::Aggregate => vec![None],
        Layout::PerService => snapshot.services.iter().filter(|s| !s.hidden && !s.one_click).map(|s| Some(s.id.clone())).collect(),
    }
}

pub fn run() -> Result<(), String> {
    let connection =
        zbus::blocking::Connection::session().map_err(|e| format!("cannot reach the session bus: {e}"))?;
    let proxy =
        LivedotProxyBlocking::new(&connection).map_err(|e| format!("cannot build the daemon proxy: {e}"))?;
    let (sender, receiver) = mpsc::channel::<()>();
    {
        let proxy = proxy.clone();
        std::thread::spawn(move || match proxy.receive_changed() {
            Ok(signals) => {
                for _ in signals {
                    if sender.send(()).is_err() {
                        return;
                    }
                }
                eprintln!("livedot tray: lost the session bus");
                std::process::exit(1);
            }
            Err(error) => eprintln!("livedot tray: cannot subscribe to Changed, polling only: {error}"),
        });
    }
    let mut trays: HashMap<Option<String>, Handle<LiveTray>> = HashMap::new();
    let mut wanted: Vec<Option<String>> = vec![None];
    let mut last: Option<Snapshot> = None;
    let mut logos = Logos::new();
    loop {
        let problem = match bus::fetch(&proxy) {
            Ok(snapshot) => {
                wanted = keys(&snapshot);
                load_logos(&mut logos, &snapshot);
                last = Some(snapshot);
                None
            }
            Err(error) => Some(error),
        };
        trays.retain(|key, handle| {
            let keep = wanted.contains(key);
            if !keep {
                handle.shutdown();
            }
            keep
        });
        for key in &wanted {
            if trays.contains_key(key) {
                continue;
            }
            let tray = LiveTray {
                root: key.clone(),
                snapshot: last.clone(),
                problem: problem.clone(),
                logos: logos.clone(),
                connection: connection.clone(),
            };
            match tray.assume_sni_available(true).spawn() {
                Ok(handle) => {
                    trays.insert(key.clone(), handle);
                }
                Err(error) => eprintln!("livedot tray: cannot register a tray item: {error}"),
            }
        }
        for handle in trays.values() {
            let snapshot = last.clone();
            let problem = problem.clone();
            let logos = logos.clone();
            handle.update(move |tray: &mut LiveTray| {
                tray.snapshot = snapshot;
                tray.problem = problem;
                tray.logos = logos;
            });
        }
        if receiver.recv_timeout(POLL).is_ok() {
            while receiver.try_recv().is_ok() {}
        }
    }
}
