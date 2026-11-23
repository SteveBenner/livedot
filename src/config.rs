use crate::model::Layout;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Copy, Debug)]
pub struct Dur(pub Duration);

impl<'de> Deserialize<'de> for Dur {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Seconds(u64),
            Text(String),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Seconds(seconds) => Ok(Dur(Duration::from_secs(seconds))),
            Raw::Text(text) => parse_duration(&text).map(Dur).map_err(serde::de::Error::custom),
        }
    }
}

fn parse_duration(text: &str) -> Result<Duration, String> {
    let text = text.trim();
    let split = text.find(|c: char| !c.is_ascii_digit()).unwrap_or(text.len());
    let (digits, unit) = text.split_at(split);
    let value: u64 = digits
        .parse()
        .map_err(|_| format!("invalid duration {text:?}: expected forms like 500ms, 5s, 2m"))?;
    match unit.trim() {
        "ms" => Ok(Duration::from_millis(value)),
        "" | "s" => Ok(Duration::from_secs(value)),
        "m" => Ok(Duration::from_secs(value * 60)),
        other => Err(format!("invalid duration unit {other:?} in {text:?}: use ms, s or m")),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    #[default]
    User,
    System,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Defaults {
    pub interval: Dur,
    pub timeout: Dur,
    pub fail_threshold: u32,
    pub flap_window: Dur,
    pub flap_threshold: u32,
    pub boot_grace: Dur,
    pub max_backoff: Dur,
    pub control_timeout: Dur,
}

impl Default for Defaults {
    fn default() -> Self {
        Defaults {
            interval: Dur(Duration::from_secs(5)),
            timeout: Dur(Duration::from_secs(3)),
            fail_threshold: 2,
            flap_window: Dur(Duration::from_secs(60)),
            flap_threshold: 3,
            boot_grace: Dur(Duration::from_secs(30)),
            max_backoff: Dur(Duration::from_secs(30)),
            control_timeout: Dur(Duration::from_secs(60)),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum CommandSpec {
    Shell(String),
    Argv(Vec<String>),
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSpec {
    pub http: Option<String>,
    pub expect_status: Option<Vec<u16>>,
    pub unit: Option<String>,
    pub scope: Option<Scope>,
    pub process: Option<String>,
    pub command: Option<CommandSpec>,
    pub interval: Option<Dur>,
    pub timeout: Option<Dur>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControlSpec {
    pub unit: Option<String>,
    pub scope: Option<Scope>,
    pub start: Option<CommandSpec>,
    pub stop: Option<CommandSpec>,
    pub restart: Option<CommandSpec>,
    pub cwd: Option<String>,
    pub timeout: Option<Dur>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ButtonSpec {
    pub name: String,
    pub label: Option<String>,
    pub icon: Option<String>,
    pub command: CommandSpec,
    pub cwd: Option<String>,
    pub timeout: Option<Dur>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PromptSpec {
    pub command: CommandSpec,
    pub cwd: Option<String>,
    pub timeout: Option<Dur>,
    #[serde(default)]
    pub buttons: Vec<ButtonSpec>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceSpec {
    pub name: String,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    #[serde(default = "enabled_default")]
    pub rollup: bool,
    #[serde(default)]
    pub one_click: bool,
    pub unit: Option<String>,
    #[serde(default)]
    pub scope: Scope,
    pub check: Option<CheckSpec>,
    pub control: Option<ControlSpec>,
    pub icon: Option<String>,
    pub url: Option<String>,
    pub prompt: Option<PromptSpec>,
    #[serde(default)]
    pub item_actions: Vec<ButtonSpec>,
    #[serde(default)]
    pub children: Vec<ServiceSpec>,
}

fn enabled_default() -> bool {
    true
}

pub fn dot_size_default() -> u32 {
    100
}

pub const DOT_SIZE_MIN: u32 = 50;
pub const DOT_SIZE_MAX: u32 = 200;

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigFile {
    #[serde(default)]
    pub layout: Layout,
    #[serde(default = "enabled_default")]
    pub shell_panel: bool,
    #[serde(default = "dot_size_default")]
    pub dot_size: u32,
    #[serde(default)]
    pub hide_healthy: bool,
    #[serde(default)]
    pub defaults: Defaults,
    pub services: Vec<ServiceSpec>,
}

#[derive(Clone, Debug)]
pub enum CheckKind {
    Http { url: String, expect_status: Option<Vec<u16>> },
    Unit { unit: String, scope: Scope },
    Process { pattern: String },
    Command { command: CommandSpec },
}

#[derive(Clone, Debug)]
pub struct Check {
    pub kind: CheckKind,
    pub interval: Duration,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub enum ControlKind {
    Unit { unit: String, scope: Scope },
    Commands {
        start: Option<CommandSpec>,
        stop: Option<CommandSpec>,
        restart: Option<CommandSpec>,
        cwd: Option<PathBuf>,
    },
}

#[derive(Clone, Debug)]
pub struct Control {
    pub kind: ControlKind,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct Button {
    pub name: String,
    pub label: String,
    pub icon: Option<String>,
    pub command: CommandSpec,
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
}

#[derive(Clone, Debug)]
pub struct Prompt {
    pub command: CommandSpec,
    pub cwd: Option<PathBuf>,
    pub timeout: Duration,
    pub buttons: Vec<Button>,
}

pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub rollup: bool,
    pub one_click: bool,
    pub check: Option<Check>,
    pub control: Option<Control>,
    pub icon: Option<PathBuf>,
    pub url: Option<String>,
    pub prompt: Option<Prompt>,
    pub item_actions: Vec<Button>,
    pub children: Vec<Node>,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub layout: Layout,
    pub shell_panel: bool,
    pub dot_size: u32,
    pub hide_healthy: bool,
    pub defaults: Defaults,
    pub services: Vec<Node>,
}

#[derive(Debug)]
pub struct ConfigError(pub String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub fn default_path() -> PathBuf {
    if let Some(path) = std::env::var_os("LIVEDOT_CONFIG") {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config"));
    base.join("livedot").join("config.yml")
}

pub fn load(path: &Path) -> Result<Config, ConfigError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ConfigError(format!("cannot read {}: {e}", path.display())))?;
    let file: ConfigFile = serde_norway::from_str(&text)
        .map_err(|e| ConfigError(format!("{}: {e}", path.display())))?;
    resolve(file)
}

fn resolve(file: ConfigFile) -> Result<Config, ConfigError> {
    let defaults = file.defaults.clone();
    if defaults.interval.0 < Duration::from_secs(1) {
        return Err(ConfigError("defaults.interval must be at least 1s".into()));
    }
    if defaults.fail_threshold == 0 || defaults.flap_threshold == 0 {
        return Err(ConfigError("defaults.fail_threshold and defaults.flap_threshold must be at least 1".into()));
    }
    if !(DOT_SIZE_MIN..=DOT_SIZE_MAX).contains(&file.dot_size) {
        return Err(ConfigError(format!("dot_size must be between {DOT_SIZE_MIN} and {DOT_SIZE_MAX}")));
    }
    if file.services.is_empty() {
        return Err(ConfigError("services is empty: declare at least one service".into()));
    }
    let services = resolve_list(&file.services, "", &defaults)?;
    Ok(Config {
        layout: file.layout,
        shell_panel: file.shell_panel,
        dot_size: file.dot_size,
        hide_healthy: file.hide_healthy,
        defaults,
        services,
    })
}

fn resolve_list(specs: &[ServiceSpec], prefix: &str, defaults: &Defaults) -> Result<Vec<Node>, ConfigError> {
    let mut seen = HashSet::new();
    let mut nodes = Vec::new();
    for spec in specs {
        let name = spec.name.trim();
        if name.is_empty() || name.contains('/') {
            return Err(ConfigError(format!(
                "service name {:?} under {:?} must be non-empty and contain no slash",
                spec.name, prefix
            )));
        }
        if !seen.insert(name.to_string()) {
            return Err(ConfigError(format!("duplicate service name {name:?} under {prefix:?}")));
        }
        let id = if prefix.is_empty() { name.to_string() } else { format!("{prefix}/{name}") };
        let check = resolve_check(spec, &id, defaults)?;
        let control = resolve_control(spec, &id, defaults)?;
        let icon = resolve_icon(spec, &id)?;
        let url = match spec.url.as_deref().map(str::trim) {
            Some(url) if url.starts_with("http://") || url.starts_with("https://") => Some(url.to_string()),
            Some(url) => {
                return Err(ConfigError(format!(
                    "service {id:?}: url {url:?} must start with http:// or https://"
                )));
            }
            None => None,
        };
        let children = resolve_list(&spec.children, &id, defaults)?;
        let prompt = match &spec.prompt {
            Some(prompt) => Some(resolve_prompt(prompt, &id)?),
            None => None,
        };
        if prompt.is_some() && (check.is_some() || control.is_some() || !children.is_empty()) {
            return Err(ConfigError(format!(
                "service {id:?}: a prompt service takes no check, unit, control or children"
            )));
        }
        if prompt.is_none() && check.is_none() && children.is_empty() {
            return Err(ConfigError(format!(
                "service {id:?} has nothing to watch: give it a check, a unit, children, or a prompt"
            )));
        }
        let item_actions = resolve_buttons(&spec.item_actions, &id, defaults.control_timeout.0)?;
        if !item_actions.is_empty() && !matches!(check.as_ref().map(|c| &c.kind), Some(CheckKind::Command { .. })) {
            return Err(ConfigError(format!("service {id:?}: item_actions needs a command check")));
        }
        if spec.one_click {
            if !prefix.is_empty() {
                return Err(ConfigError(format!("service {id:?}: one_click applies only to a top-level service")));
            }
            if url.is_some() {
                return Err(ConfigError(format!(
                    "service {id:?}: one_click cannot be combined with a url, which already claims the click"
                )));
            }
            if item_actions.is_empty() {
                return Err(ConfigError(format!("service {id:?}: one_click needs item_actions")));
            }
        }
        nodes.push(Node {
            id,
            name: name.to_string(),
            enabled: spec.enabled,
            rollup: spec.rollup,
            one_click: spec.one_click,
            check,
            control,
            icon,
            url,
            prompt,
            item_actions,
            children,
        });
    }
    Ok(nodes)
}

fn resolve_prompt(spec: &PromptSpec, id: &str) -> Result<Prompt, ConfigError> {
    Ok(Prompt {
        command: spec.command.clone(),
        cwd: spec.cwd.as_ref().map(PathBuf::from),
        timeout: spec.timeout.map(|d| d.0).unwrap_or(PROMPT_TIMEOUT),
        buttons: resolve_buttons(&spec.buttons, id, PROMPT_TIMEOUT)?,
    })
}

fn resolve_buttons(specs: &[ButtonSpec], id: &str, default_timeout: Duration) -> Result<Vec<Button>, ConfigError> {
    let mut seen = HashSet::new();
    let mut buttons = Vec::new();
    for button in specs {
        let name = button.name.trim();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Err(ConfigError(format!(
                "service {id:?}: button name {:?} must be letters, digits, - or _",
                button.name
            )));
        }
        if !seen.insert(name.to_string()) {
            return Err(ConfigError(format!("service {id:?}: duplicate button name {name:?}")));
        }
        buttons.push(Button {
            name: name.to_string(),
            label: button.label.clone().unwrap_or_else(|| name.to_string()),
            icon: button.icon.clone(),
            command: button.command.clone(),
            cwd: button.cwd.as_ref().map(PathBuf::from),
            timeout: button.timeout.map(|d| d.0).unwrap_or(default_timeout),
        });
    }
    Ok(buttons)
}

fn resolve_icon(spec: &ServiceSpec, id: &str) -> Result<Option<PathBuf>, ConfigError> {
    let Some(raw) = &spec.icon else {
        return Ok(None);
    };
    let path = match raw.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(raw),
    };
    if !path.is_absolute() {
        return Err(ConfigError(format!(
            "service {id:?}: icon {raw:?} must be an absolute path or start with ~/"
        )));
    }
    Ok(Some(path))
}

fn resolve_check(spec: &ServiceSpec, id: &str, defaults: &Defaults) -> Result<Option<Check>, ConfigError> {
    let (kind, interval, timeout) = match &spec.check {
        Some(check) => {
            let given = [check.http.is_some(), check.unit.is_some(), check.process.is_some(), check.command.is_some()]
                .iter()
                .filter(|b| **b)
                .count();
            if given != 1 {
                return Err(ConfigError(format!(
                    "service {id:?}: check needs exactly one of http, unit, process, command"
                )));
            }
            let kind = if let Some(url) = &check.http {
                if !(url.starts_with("http://") || url.starts_with("https://")) {
                    return Err(ConfigError(format!(
                        "service {id:?}: check.http {url:?} must start with http:// or https://"
                    )));
                }
                CheckKind::Http { url: url.clone(), expect_status: check.expect_status.clone() }
            } else if let Some(unit) = &check.unit {
                CheckKind::Unit { unit: unit.clone(), scope: check.scope.unwrap_or(spec.scope) }
            } else if let Some(command) = &check.command {
                let empty = match command {
                    CommandSpec::Shell(line) => line.trim().is_empty(),
                    CommandSpec::Argv(argv) => argv.is_empty(),
                };
                if empty {
                    return Err(ConfigError(format!("service {id:?}: check.command is empty")));
                }
                CheckKind::Command { command: command.clone() }
            } else {
                let pattern = check.process.clone().unwrap_or_default();
                if pattern.trim().is_empty() {
                    return Err(ConfigError(format!("service {id:?}: check.process is empty")));
                }
                CheckKind::Process { pattern }
            };
            if check.expect_status.is_some() && check.http.is_none() {
                return Err(ConfigError(format!(
                    "service {id:?}: expect_status only applies to an http check"
                )));
            }
            (kind, check.interval, check.timeout)
        }
        None => match &spec.unit {
            Some(unit) => (CheckKind::Unit { unit: unit.clone(), scope: spec.scope }, None, None),
            None => return Ok(None),
        },
    };
    let interval = interval.map(|d| d.0).unwrap_or(defaults.interval.0);
    if interval < Duration::from_secs(1) {
        return Err(ConfigError(format!("service {id:?}: check.interval must be at least 1s")));
    }
    let timeout = timeout.map(|d| d.0).unwrap_or(defaults.timeout.0);
    Ok(Some(Check { kind, interval, timeout }))
}

fn resolve_control(spec: &ServiceSpec, id: &str, defaults: &Defaults) -> Result<Option<Control>, ConfigError> {
    let (kind, timeout) = match &spec.control {
        Some(control) => {
            let has_commands = control.start.is_some() || control.stop.is_some() || control.restart.is_some();
            let kind = match (&control.unit, has_commands) {
                (Some(_), true) => {
                    return Err(ConfigError(format!(
                        "service {id:?}: control takes a unit or commands, not both"
                    )));
                }
                (Some(unit), false) => {
                    ControlKind::Unit { unit: unit.clone(), scope: control.scope.unwrap_or(spec.scope) }
                }
                (None, true) => ControlKind::Commands {
                    start: control.start.clone(),
                    stop: control.stop.clone(),
                    restart: control.restart.clone(),
                    cwd: control.cwd.as_ref().map(PathBuf::from),
                },
                (None, false) => {
                    return Err(ConfigError(format!(
                        "service {id:?}: control needs a unit or at least one of start, stop, restart"
                    )));
                }
            };
            (kind, control.timeout)
        }
        None => match &spec.unit {
            Some(unit) => (ControlKind::Unit { unit: unit.clone(), scope: spec.scope }, None),
            None => return Ok(None),
        },
    };
    Ok(Some(Control { kind, timeout: timeout.map(|d| d.0).unwrap_or(defaults.control_timeout.0) }))
}
