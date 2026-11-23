use crate::config::{Check, CheckKind, CommandSpec, Scope};
use serde::Deserialize;
use std::collections::HashSet;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const FEED_BYTES: u64 = 64 * 1024;
const FEED_ITEMS: usize = 20;
const FEED_TEXT: usize = 120;
const LABEL_TEXT: usize = 32;
const ITEM_ID_MAX: usize = 128;

#[derive(Clone, Debug, PartialEq)]
pub struct FeedItem {
    pub id: String,
    pub label: String,
    pub detail: Option<String>,
    pub actions: Option<Vec<String>>,
    pub checked: Option<bool>,
}

#[derive(Deserialize)]
struct Feed {
    status: Option<String>,
    detail: Option<String>,
    label: Option<String>,
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RawItem {
    id: String,
    label: Option<String>,
    detail: Option<String>,
    actions: Option<Vec<String>>,
    checked: Option<bool>,
}

pub fn valid_item_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= ITEM_ID_MAX
        && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

pub fn command_for(spec: &CommandSpec) -> Result<Command, String> {
    match spec {
        CommandSpec::Shell(line) => {
            let mut command = Command::new("sh");
            command.args(["-c", line]);
            Ok(command)
        }
        CommandSpec::Argv(argv) => {
            let Some((program, rest)) = argv.split_first() else {
                return Err("empty command".into());
            };
            let mut command = Command::new(program);
            command.args(rest);
            Ok(command)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Pass(String),
    Fail(String),
    Starting(String),
    Unknown(String),
}

impl Outcome {
    pub fn detail(&self) -> &str {
        match self {
            Outcome::Pass(d) | Outcome::Fail(d) | Outcome::Starting(d) | Outcome::Unknown(d) => d,
        }
    }
}

pub struct ProbeResult {
    pub outcome: Outcome,
    pub pushback: bool,
    pub retry_after: Option<Duration>,
    pub items: Option<Vec<FeedItem>>,
    pub label: Option<String>,
}

impl From<Outcome> for ProbeResult {
    fn from(outcome: Outcome) -> Self {
        ProbeResult { outcome, pushback: false, retry_after: None, items: None, label: None }
    }
}

pub struct Prober {
    id: String,
    check: Check,
    agent: Option<ureq::Agent>,
}

pub fn wait_timeout(child: &mut Child, timeout: Duration, kill_group: bool) -> std::io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            if kill_group {
                unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
            }
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

pub fn systemctl(scope: Scope) -> Command {
    let mut command = Command::new("systemctl");
    if scope == Scope::User {
        command.arg("--user");
    }
    command
}

impl Prober {
    pub fn new(id: &str, check: Check) -> Self {
        let agent = match &check.kind {
            CheckKind::Http { .. } => {
                let config = ureq::Agent::config_builder()
                    .timeout_global(Some(check.timeout))
                    .max_redirects(0)
                    .max_redirects_will_error(false)
                    .http_status_as_error(false)
                    .user_agent(concat!("livedot/", env!("CARGO_PKG_VERSION")))
                    .build();
                Some(ureq::Agent::new_with_config(config))
            }
            _ => None,
        };
        Prober { id: id.to_string(), check, agent }
    }

    pub fn probe(&self) -> ProbeResult {
        match &self.check.kind {
            CheckKind::Http { url, expect_status } => self.http(url, expect_status.as_deref()),
            CheckKind::Unit { unit, scope } => self.unit(unit, *scope).into(),
            CheckKind::Process { pattern } => process(pattern).into(),
            CheckKind::Command { command } => self.command(command),
        }
    }

    fn command(&self, spec: &CommandSpec) -> ProbeResult {
        let mut command = match command_for(spec) {
            Ok(command) => command,
            Err(error) => return Outcome::Unknown(error).into(),
        };
        let spawned = command
            .env("LIVEDOT_SERVICE", &self.id)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => return Outcome::Unknown(format!("cannot run the check command: {error}")).into(),
        };
        let reader = child.stdout.take().map(|stdout| {
            std::thread::spawn(move || {
                let mut stdout = stdout;
                let mut text = Vec::new();
                let _ = (&mut stdout).take(FEED_BYTES).read_to_end(&mut text);
                let _ = std::io::copy(&mut stdout, &mut std::io::sink());
                String::from_utf8_lossy(&text).into_owned()
            })
        });
        let waited = wait_timeout(&mut child, self.check.timeout, true);
        let text = reader.and_then(|handle| handle.join().ok()).unwrap_or_default();
        let status = match waited {
            Ok(Some(status)) => status,
            Ok(None) => {
                return Outcome::Fail(format!("killed after {}s timeout", self.check.timeout.as_secs())).into();
            }
            Err(error) => return Outcome::Unknown(format!("wait failed: {error}")).into(),
        };
        let feed = serde_json::from_str::<Feed>(text.trim()).ok();
        let first_line = cut(text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or(""));
        let detail = feed
            .as_ref()
            .and_then(|feed| feed.detail.as_deref())
            .map(cut)
            .filter(|detail| !detail.is_empty())
            .unwrap_or_else(|| if feed.is_none() && !first_line.is_empty() { first_line } else { format!("exit {}", status.code().unwrap_or(-1)) });
        let outcome = if !status.success() {
            Outcome::Fail(detail)
        } else {
            match feed.as_ref().and_then(|feed| feed.status.as_deref()) {
                None | Some("up") => Outcome::Pass(detail),
                Some("down") => Outcome::Fail(detail),
                Some("booting") => Outcome::Starting(detail),
                Some("unsure") => Outcome::Unknown(detail),
                Some(other) => Outcome::Unknown(format!("unknown status {:?}: {detail}", cut(other))),
            }
        };
        let label = feed
            .as_ref()
            .and_then(|feed| feed.label.as_deref())
            .map(|label| clip(label, LABEL_TEXT))
            .filter(|label| !label.is_empty());
        let items = feed.map(|feed| items(feed.items)).unwrap_or_default();
        ProbeResult { outcome, pushback: false, retry_after: None, items: Some(items), label }
    }

    fn http(&self, url: &str, expect: Option<&[u16]>) -> ProbeResult {
        let Some(agent) = &self.agent else {
            return Outcome::Unknown("no http client".into()).into();
        };
        match agent.get(url).call() {
            Ok(response) => {
                let code = response.status().as_u16();
                let passed = match expect {
                    Some(list) => list.contains(&code),
                    None => (200..300).contains(&code),
                };
                if passed {
                    return Outcome::Pass(format!("HTTP {code}")).into();
                }
                let pushback = code == 429 || code == 503;
                let retry_after = if pushback {
                    response
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.trim().parse::<u64>().ok())
                        .map(Duration::from_secs)
                } else {
                    None
                };
                ProbeResult { outcome: Outcome::Fail(format!("HTTP {code}")), pushback, retry_after, items: None, label: None }
            }
            Err(error) => Outcome::Fail(short(&error.to_string())).into(),
        }
    }

    fn unit(&self, unit: &str, scope: Scope) -> Outcome {
        let spawned = systemctl(scope)
            .args(["show", unit, "--property=ActiveState", "--property=SubState"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let mut child = match spawned {
            Ok(child) => child,
            Err(error) => return Outcome::Unknown(format!("cannot run systemctl: {error}")),
        };
        match wait_timeout(&mut child, self.check.timeout, false) {
            Ok(Some(status)) if status.success() => {}
            Ok(Some(status)) => return Outcome::Unknown(format!("systemctl show exited {status}")),
            Ok(None) => return Outcome::Unknown("systemctl show timed out".into()),
            Err(error) => return Outcome::Unknown(format!("systemctl show failed: {error}")),
        }
        let mut text = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            let _ = stdout.read_to_string(&mut text);
        }
        let field = |key: &str| {
            text.lines()
                .find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix('=')))
                .unwrap_or("")
                .to_string()
        };
        let active = field("ActiveState");
        let sub = field("SubState");
        let detail = format!("{active} ({sub})");
        match active.as_str() {
            "active" => Outcome::Pass(detail),
            "activating" | "reloading" => Outcome::Starting(detail),
            "" => Outcome::Unknown("systemctl show returned no state".into()),
            _ => Outcome::Fail(detail),
        }
    }
}

fn process(pattern: &str) -> Outcome {
    let entries = match std::fs::read_dir("/proc") {
        Ok(entries) => entries,
        Err(error) => return Outcome::Unknown(format!("cannot read /proc: {error}")),
    };
    let own = std::process::id();
    let mut count = 0usize;
    for entry in entries.flatten() {
        let Some(pid) = entry.file_name().to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        if pid == own {
            continue;
        }
        let Ok(raw) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let line = String::from_utf8_lossy(&raw).replace('\0', " ");
        if line.contains(pattern) {
            count += 1;
        }
    }
    match count {
        0 => Outcome::Fail("no matching process".into()),
        1 => Outcome::Pass("1 process".into()),
        n => Outcome::Pass(format!("{n} processes")),
    }
}

fn short(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() > 80 {
        let cut: String = line.chars().take(79).collect();
        format!("{cut}…")
    } else {
        line.to_string()
    }
}

fn items(values: Vec<serde_json::Value>) -> Vec<FeedItem> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter_map(|value| serde_json::from_value::<RawItem>(value).ok())
        .filter(|raw| valid_item_id(&raw.id) && seen.insert(raw.id.clone()))
        .take(FEED_ITEMS)
        .map(|raw| FeedItem {
            label: raw.label.as_deref().map(cut).filter(|label| !label.is_empty()).unwrap_or_else(|| raw.id.clone()),
            detail: raw.detail.as_deref().map(cut).filter(|detail| !detail.is_empty()),
            actions: raw.actions,
            checked: raw.checked,
            id: raw.id,
        })
        .collect()
}

fn cut(text: &str) -> String {
    clip(text, FEED_TEXT)
}

fn clip(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.chars().count() > max {
        let kept: String = line.chars().take(max - 1).collect();
        format!("{kept}…")
    } else {
        line.to_string()
    }
}
