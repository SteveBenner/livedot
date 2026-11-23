use crate::config::{Config, ICON_SIZE_MAX, ICON_SIZE_MIN, Node};
use crate::control::{self, Action};
use crate::model::{ActionResult, ButtonView, ItemView, PanelPosition, ServiceView, Snapshot, Status};
use crate::probe::{FeedItem, Outcome, ProbeResult, Prober};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RETRY_AFTER_CAP: Duration = Duration::from_secs(300);
const MIN_DELAY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct LeafState {
    last: Option<Outcome>,
    last_pass: Option<bool>,
    fails: u32,
    pushbacks: u32,
    flips: VecDeque<Instant>,
    boot_until: Option<Instant>,
    quiet_until: Option<Instant>,
    items: Vec<FeedItem>,
    label: Option<String>,
}

#[derive(Default)]
struct Waker {
    flag: Mutex<bool>,
    cond: Condvar,
}

impl Waker {
    fn wait(&self, delay: Duration) {
        let guard = self.flag.lock().unwrap();
        let (mut guard, _) = self.cond.wait_timeout_while(guard, delay, |woken| !*woken).unwrap();
        let _ = std::mem::replace(&mut *guard, false);
    }

    fn wake(&self) {
        let _ = std::mem::replace(&mut *self.flag.lock().unwrap(), true);
        self.cond.notify_all();
    }
}

pub struct Shared {
    pub config: Config,
    pub probes_disabled: bool,
    disabled: HashSet<String>,
    states: Mutex<HashMap<String, LeafState>>,
    wakers: HashMap<String, Arc<Waker>>,
    pub busy: Mutex<HashSet<String>>,
    pub results: Mutex<HashMap<String, ActionResult>>,
    hidden: Mutex<HashSet<String>>,
    hidden_path: PathBuf,
    config_path: PathBuf,
    config_lock: Mutex<()>,
    icon_size: AtomicU32,
    hide_healthy: AtomicBool,
    panel_position: Mutex<PanelPosition>,
    notify: Sender<()>,
}

fn today() -> String {
    let days = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() / 86400).unwrap_or(0) as i64;
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn edit_config_line(path: &std::path::Path, key: &str, value: &str, after: &[&str]) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("cannot write {}: {error}", path.display());
    let text = std::fs::read_to_string(path).map_err(failed)?;
    let mut backup = path.to_path_buf().into_os_string();
    backup.push(format!(".bak-{}-livedot", today()));
    let backup = PathBuf::from(backup);
    if !backup.exists() {
        std::fs::copy(path, &backup).map_err(failed)?;
    }
    let line = format!("{key}: {value}");
    let starts = |candidate: &str, name: &str| candidate.starts_with(&format!("{name}:"));
    let mut lines: Vec<&str> = text.split_inclusive('\n').collect();
    let ending = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let terminated = |row: &str| if row.ends_with('\n') { format!("{line}{ending}") } else { line.clone() };
    let mut out = String::with_capacity(text.len() + line.len() + 2);
    if let Some(index) = lines.iter().position(|row| starts(row, key)) {
        let replacement = terminated(lines[index]);
        for (at, row) in lines.drain(..).enumerate() {
            out.push_str(if at == index { &replacement } else { row });
        }
    } else {
        let anchor = after.iter().find_map(|name| lines.iter().position(|row| starts(row, name)));
        let at = anchor.map(|index| index + 1).unwrap_or(0);
        let mut inserted = false;
        for (index, row) in lines.iter().enumerate() {
            if index == at {
                out.push_str(&format!("{line}{ending}"));
                inserted = true;
            }
            out.push_str(row);
            if index + 1 == at && !row.ends_with('\n') {
                out.push_str(ending);
            }
        }
        if !inserted {
            out.push_str(&format!("{line}{ending}"));
        }
    }
    let mut temporary = path.to_path_buf().into_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, out).map_err(failed)?;
    std::fs::rename(&temporary, path).map_err(failed)
}

fn hidden_path() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local").join("state"));
    base.join("livedot").join("hidden")
}

fn load_hidden(path: &std::path::Path) -> HashSet<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => text.lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashSet::new(),
        Err(error) => {
            eprintln!("livedot: cannot read {}: {error}; showing every service", path.display());
            HashSet::new()
        }
    }
}

fn save_hidden(path: &std::path::Path, hidden: &HashSet<String>) -> Result<(), String> {
    let failed = |error: std::io::Error| format!("cannot write {}: {error}", path.display());
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    let mut ids: Vec<&String> = hidden.iter().collect();
    ids.sort();
    let text: String = ids.iter().map(|id| format!("{id}\n")).collect();
    let mut temporary = path.to_path_buf().into_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, text).map_err(failed)?;
    std::fs::rename(&temporary, path).map_err(failed)
}

pub fn root_of(id: &str) -> &str {
    id.split('/').next().unwrap_or(id)
}

pub fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Node> {
    for node in nodes {
        if node.id == id {
            return Some(node);
        }
        if let Some(found) = find(&node.children, id) {
            return Some(found);
        }
    }
    None
}

pub fn leaf_ids(node: &Node, out: &mut Vec<String>) {
    if node.check.is_some() {
        out.push(node.id.clone());
    }
    for child in &node.children {
        leaf_ids(child, out);
    }
}

impl Shared {
    pub fn new(config: Config, config_path: PathBuf, probes_disabled: bool, notify: Sender<()>) -> Arc<Shared> {
        fn collect(nodes: &[Node], parent_enabled: bool, disabled: &mut HashSet<String>, checked: &mut Vec<String>) {
            for node in nodes {
                let enabled = parent_enabled && node.enabled;
                if !enabled {
                    disabled.insert(node.id.clone());
                }
                if node.check.is_some() {
                    checked.push(node.id.clone());
                }
                collect(&node.children, enabled, disabled, checked);
            }
        }
        let mut disabled = HashSet::new();
        let mut checked = Vec::new();
        collect(&config.services, true, &mut disabled, &mut checked);
        let wakers = checked.iter().map(|id| (id.clone(), Arc::new(Waker::default()))).collect();
        let states = checked.iter().map(|id| (id.clone(), LeafState::default())).collect();
        let hidden_path = hidden_path();
        let hidden = load_hidden(&hidden_path);
        let icon_size = AtomicU32::new(config.icon_size);
        let hide_healthy = AtomicBool::new(config.hide_healthy);
        let panel_position = Mutex::new(config.panel_position);
        Arc::new(Shared {
            config,
            probes_disabled,
            disabled,
            states: Mutex::new(states),
            wakers,
            busy: Mutex::new(HashSet::new()),
            results: Mutex::new(HashMap::new()),
            hidden: Mutex::new(hidden),
            hidden_path,
            config_path,
            config_lock: Mutex::new(()),
            icon_size,
            hide_healthy,
            panel_position,
            notify,
        })
    }

    pub fn set_panel_position(&self, position: &str) -> Result<(), String> {
        let Some(position) = PanelPosition::parse(position) else {
            return Err(format!("panel position must be left, center or right, not {position:?}"));
        };
        let _guard = self.config_lock.lock().unwrap();
        edit_config_line(&self.config_path, "panel_position", position.word(), &["hide_healthy", "icon_size", "layout"])?;
        self.panel_position.lock().unwrap().clone_from(&position);
        eprintln!("livedot: panel position {}", position.word());
        self.changed();
        Ok(())
    }

    pub fn set_icon_size(&self, percent: u32) -> Result<(), String> {
        if !(ICON_SIZE_MIN..=ICON_SIZE_MAX).contains(&percent) {
            return Err(format!("icon size must be between {ICON_SIZE_MIN} and {ICON_SIZE_MAX}"));
        }
        let _guard = self.config_lock.lock().unwrap();
        edit_config_line(&self.config_path, "icon_size", &percent.to_string(), &["layout"])?;
        self.icon_size.store(percent, Ordering::SeqCst);
        eprintln!("livedot: icon size {percent}%");
        self.changed();
        Ok(())
    }

    pub fn set_hide_healthy(&self, hide: bool) -> Result<(), String> {
        let _guard = self.config_lock.lock().unwrap();
        edit_config_line(&self.config_path, "hide_healthy", if hide { "true" } else { "false" }, &["icon_size", "layout"])?;
        self.hide_healthy.store(hide, Ordering::SeqCst);
        eprintln!("livedot: hide healthy {}", if hide { "on" } else { "off" });
        self.changed();
        Ok(())
    }

    pub fn set_shown(&self, id: &str, shown: bool) -> Result<(), String> {
        if !self.config.services.iter().any(|node| node.id == id) {
            return Err(format!("{id:?} is not a top-level service; only those have a place in the bar"));
        }
        let mut hidden = self.hidden.lock().unwrap();
        if hidden.contains(id) != shown {
            return Ok(());
        }
        let mut next = hidden.clone();
        if shown {
            next.remove(id);
        } else {
            next.insert(id.to_string());
        }
        save_hidden(&self.hidden_path, &next)?;
        *hidden = next;
        drop(hidden);
        eprintln!("livedot: {id} {} in the bar", if shown { "shown" } else { "hidden" });
        self.changed();
        Ok(())
    }

    pub fn start(self: &Arc<Self>) {
        fn spawn(shared: &Arc<Shared>, nodes: &[Node]) {
            for node in nodes {
                if let Some(check) = &node.check {
                    if !shared.probes_disabled && !shared.disabled.contains(&node.id) {
                        let shared = Arc::clone(shared);
                        let id = node.id.clone();
                        let interval = check.interval;
                        let prober = Prober::new(&id, check.clone());
                        std::thread::spawn(move || {
                            let waker = Arc::clone(&shared.wakers[&id]);
                            loop {
                                let result = prober.probe();
                                let delay = shared.record(&id, interval, result);
                                shared.changed();
                                waker.wait(delay);
                            }
                        });
                    }
                }
                spawn(shared, &node.children);
            }
        }
        spawn(self, &self.config.services);
    }

    pub fn changed(&self) {
        let _ = self.notify.send(());
    }

    fn record(&self, id: &str, interval: Duration, result: ProbeResult) -> Duration {
        let defaults = &self.config.defaults;
        let now = Instant::now();
        let mut states = self.states.lock().unwrap();
        let state = states.entry(id.to_string()).or_default();
        if let Some(items) = result.items {
            state.items = items;
            state.label = result.label;
        }
        let pass = matches!(result.outcome, Outcome::Pass(_));
        if !matches!(result.outcome, Outcome::Unknown(_)) {
            let quiet = state.quiet_until.is_some_and(|until| now < until);
            if state.last_pass.is_some_and(|previous| previous != pass) && !quiet {
                state.flips.push_back(now);
            }
            state.last_pass = Some(pass);
            if pass {
                state.fails = 0;
            } else {
                state.fails = state.fails.saturating_add(1);
            }
        }
        while state.flips.front().is_some_and(|at| now.duration_since(*at) > defaults.flap_window.0) {
            state.flips.pop_front();
        }
        state.last = Some(result.outcome);
        let booting = state.boot_until.is_some_and(|until| now < until);
        state.pushbacks = if result.pushback { state.pushbacks.saturating_add(1) } else { 0 };
        let mut delay = interval;
        if state.pushbacks > 0 && !booting {
            let factor = 1u32 << state.pushbacks.min(16);
            delay = interval.saturating_mul(factor).min(defaults.max_backoff.0.max(interval));
        }
        let delay = jitter(delay).max(MIN_DELAY);
        match result.retry_after {
            Some(retry_after) => delay.max(retry_after.min(RETRY_AFTER_CAP)),
            None => delay,
        }
    }

    pub fn mark_action(&self, node: &Node, action: Action) {
        let grace = self.config.defaults.boot_grace.0;
        let now = Instant::now();
        let mut ids = Vec::new();
        leaf_ids(node, &mut ids);
        let mut states = self.states.lock().unwrap();
        for id in ids {
            let state = states.entry(id).or_default();
            state.quiet_until = Some(now + grace);
            state.flips.clear();
            match action {
                Action::Start | Action::Restart => {
                    state.boot_until = Some(now + grace);
                    state.fails = 0;
                }
                Action::Stop => state.boot_until = None,
            }
        }
    }

    pub fn wake(&self, node: &Node) {
        let mut ids = Vec::new();
        leaf_ids(node, &mut ids);
        for id in ids {
            if let Some(waker) = self.wakers.get(&id) {
                waker.wake();
            }
        }
    }

    fn leaf_status(&self, id: &str, states: &HashMap<String, LeafState>, now: Instant) -> (Status, String) {
        if self.probes_disabled || self.disabled.contains(id) {
            return (Status::Unsure, "probing disabled".into());
        }
        let defaults = &self.config.defaults;
        let Some(state) = states.get(id) else {
            return (Status::Unsure, "not checked yet".into());
        };
        let booting = state.boot_until.is_some_and(|until| now < until);
        let Some(last) = &state.last else {
            return if booting {
                (Status::Booting, "starting".into())
            } else {
                (Status::Unsure, "not checked yet".into())
            };
        };
        let flips = state
            .flips
            .iter()
            .filter(|at| now.duration_since(**at) <= defaults.flap_window.0)
            .count();
        if flips as u32 >= defaults.flap_threshold {
            return (
                Status::Flapping,
                format!("{flips} flips in {}s, last {}", defaults.flap_window.0.as_secs(), last.detail()),
            );
        }
        match last {
            Outcome::Pass(detail) => (Status::Up, detail.clone()),
            Outcome::Starting(detail) => (Status::Booting, detail.clone()),
            Outcome::Unknown(detail) => (Status::Unsure, detail.clone()),
            Outcome::Fail(detail) => {
                if booting {
                    (Status::Booting, format!("starting, {detail}"))
                } else if state.fails < defaults.fail_threshold {
                    (Status::Unsure, format!("{detail}, confirming"))
                } else {
                    (Status::Down, detail.clone())
                }
            }
        }
    }

    fn prompt_status(
        &self,
        node: &Node,
        busy: &HashSet<String>,
        results: &HashMap<String, ActionResult>,
    ) -> (Status, String) {
        if self.disabled.contains(&node.id) {
            return (Status::Unsure, "disabled".into());
        }
        if busy.contains(root_of(&node.id)) {
            return (Status::Working, "working".into());
        }
        match results.get(&node.id) {
            Some(result) if !result.ok => (Status::Down, format!("last run failed: {}", result.message)),
            Some(_) => (Status::Up, "last run ok".into()),
            None => (Status::Up, "idle".into()),
        }
    }

    pub fn is_disabled(&self, id: &str) -> bool {
        self.disabled.contains(id)
    }

    pub fn offers(&self, node: &Node, item: &str, action: &str) -> bool {
        let states = self.states.lock().unwrap();
        states
            .get(&node.id)
            .and_then(|state| state.items.iter().find(|candidate| candidate.id == item))
            .is_some_and(|found| item_actions(node, found).iter().any(|button| button.name == action))
    }

    fn view(
        &self,
        node: &Node,
        states: &HashMap<String, LeafState>,
        busy: &HashSet<String>,
        results: &HashMap<String, ActionResult>,
        now: Instant,
    ) -> ServiceView {
        let children: Vec<ServiceView> =
            node.children.iter().map(|child| self.view(child, states, busy, results, now)).collect();
        let own = match &node.prompt {
            Some(_) => Some(self.prompt_status(node, busy, results)),
            None => node.check.as_ref().map(|_| self.leaf_status(&node.id, states, now)),
        };
        let rolled: Vec<&ServiceView> =
            node.children.iter().zip(&children).filter(|(child, _)| child.rollup).map(|(_, view)| view).collect();
        let status = if !children.is_empty() && own.is_none() && rolled.is_empty() {
            Status::Unsure
        } else {
            Status::worst(own.iter().map(|(s, _)| *s).chain(rolled.iter().map(|c| c.status)))
                .unwrap_or(Status::Unsure)
        };
        let detail = if children.is_empty() {
            own.map(|(_, detail)| detail).unwrap_or_default()
        } else if own.is_none() && rolled.is_empty() {
            "no child rolls up".to_string()
        } else {
            let mut parts: Vec<String> = Vec::new();
            if let Some((own_status, detail)) = &own {
                if *own_status != Status::Up {
                    parts.push(detail.clone());
                }
            }
            parts.extend(
                rolled
                    .iter()
                    .filter(|c| c.status != Status::Up)
                    .map(|c| format!("{} {}", c.name, c.status.word())),
            );
            if parts.is_empty() { "all up".to_string() } else { parts.join(", ") }
        };
        ServiceView {
            id: node.id.clone(),
            name: node.name.clone(),
            status,
            color: status.color(),
            detail,
            label: states.get(&node.id).and_then(|state| state.label.clone()),
            icon: node.icon.as_ref().map(|path| path.display().to_string()),
            url: node.url.clone(),
            actions: control::available(node),
            buttons: node
                .prompt
                .iter()
                .flat_map(|prompt| &prompt.buttons)
                .map(|button| ButtonView {
                    name: button.name.clone(),
                    label: button.label.clone(),
                    icon: button.icon.clone(),
                })
                .collect(),
            items: states
                .get(&node.id)
                .map(|state| {
                    state
                        .items
                        .iter()
                        .map(|item| ItemView {
                            id: item.id.clone(),
                            label: item.label.clone(),
                            detail: item.detail.clone(),
                            checked: item.checked,
                            actions: item_actions(node, item),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            busy: busy.contains(root_of(&node.id)),
            last_action: results.get(&node.id).cloned(),
            children,
            hidden: false,
            rollup: node.rollup,
            one_click: node.one_click,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let now = Instant::now();
        let busy = self.busy.lock().unwrap().clone();
        let results = self.results.lock().unwrap().clone();
        let states = self.states.lock().unwrap();
        let hidden = self.hidden.lock().unwrap().clone();
        let mut services: Vec<ServiceView> =
            self.config.services.iter().map(|node| self.view(node, &states, &busy, &results, now)).collect();
        for view in &mut services {
            view.hidden = hidden.contains(&view.id);
        }
        let status = Status::worst(
            self.config
                .services
                .iter()
                .zip(&services)
                .filter(|(node, view)| node.rollup && !view.hidden)
                .map(|(_, view)| view.status),
        )
        .unwrap_or(Status::Unsure);
        Snapshot {
            layout: self.config.layout,
            shell_panel: self.config.shell_panel,
            icon_size: self.icon_size.load(Ordering::SeqCst),
            icon_size_min: ICON_SIZE_MIN,
            icon_size_max: ICON_SIZE_MAX,
            hide_healthy: self.hide_healthy.load(Ordering::SeqCst),
            panel_position: *self.panel_position.lock().unwrap(),
            status,
            color: status.color(),
            services,
        }
    }
}

fn item_actions(node: &Node, item: &FeedItem) -> Vec<ButtonView> {
    node.item_actions
        .iter()
        .filter(|button| item.actions.as_ref().is_none_or(|names| names.iter().any(|name| name == &button.name)))
        .map(|button| ButtonView { name: button.name.clone(), label: button.label.clone(), icon: button.icon.clone() })
        .collect()
}

fn jitter(delay: Duration) -> Duration {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    let permille = 900 + (nanos % 201) as u64;
    Duration::from_millis((delay.as_millis() as u64).saturating_mul(permille) / 1000)
}
