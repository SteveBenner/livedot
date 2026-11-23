use crate::config::{CommandSpec, ControlKind, Node, Prompt, Scope};
use crate::model::ActionResult;
use crate::monitor::{Shared, find, root_of};
use crate::probe::{command_for, systemctl, valid_item_id, wait_timeout};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    pub const ALL: [Action; 3] = [Action::Start, Action::Stop, Action::Restart];

    pub fn word(self) -> &'static str {
        match self {
            Action::Start => "start",
            Action::Stop => "stop",
            Action::Restart => "restart",
        }
    }

    pub fn parse(text: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.word() == text)
    }
}

enum StepKind {
    Systemctl { scope: Scope, verb: &'static str, unit: String },
    Run { command: CommandSpec, cwd: Option<PathBuf> },
}

pub struct Step {
    id: String,
    kind: StepKind,
    timeout: Duration,
}

impl Step {
    fn label(&self) -> String {
        match &self.kind {
            StepKind::Systemctl { verb, unit, .. } => format!("{}: systemctl {verb} {unit}", self.id),
            StepKind::Run { command: CommandSpec::Shell(line), .. } => format!("{}: {line}", self.id),
            StepKind::Run { command: CommandSpec::Argv(argv), .. } => format!("{}: {}", self.id, argv.join(" ")),
        }
    }

    fn run(&self) -> Result<(), String> {
        let mut command = match &self.kind {
            StepKind::Systemctl { scope, verb, unit } => {
                let mut command = systemctl(*scope);
                command.args([verb, unit.as_str()]);
                command
            }
            StepKind::Run { command: spec, cwd } => {
                let mut command = match spec {
                    CommandSpec::Shell(line) => {
                        let mut command = Command::new("sh");
                        command.args(["-c", line]);
                        command
                    }
                    CommandSpec::Argv(argv) => {
                        let Some((program, rest)) = argv.split_first() else {
                            return Err("empty command".into());
                        };
                        let mut command = Command::new(program);
                        command.args(rest);
                        command
                    }
                };
                if let Some(cwd) = cwd {
                    command.current_dir(cwd);
                }
                command
            }
        };
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .process_group(0)
            .spawn()
            .map_err(|e| format!("cannot run: {e}"))?;
        match wait_timeout(&mut child, self.timeout, true) {
            Ok(Some(status)) if status.success() => Ok(()),
            Ok(Some(status)) => Err(format!("{status}")),
            Ok(None) => Err(format!("killed after {}s timeout", self.timeout.as_secs())),
            Err(error) => Err(format!("wait failed: {error}")),
        }
    }
}

pub fn plan(node: &Node, action: Action) -> Vec<Step> {
    if let Some(control) = &node.control {
        let step = |kind| Step { id: node.id.clone(), kind, timeout: control.timeout };
        return match &control.kind {
            ControlKind::Unit { unit, scope } => {
                vec![step(StepKind::Systemctl { scope: *scope, verb: action.word(), unit: unit.clone() })]
            }
            ControlKind::Commands { start, stop, restart, cwd } => {
                let run = |command: &CommandSpec| step(StepKind::Run { command: command.clone(), cwd: cwd.clone() });
                match action {
                    Action::Start => start.iter().map(run).collect(),
                    Action::Stop => stop.iter().map(run).collect(),
                    Action::Restart => match (restart, start, stop) {
                        (Some(restart), _, _) => vec![run(restart)],
                        (None, Some(start), Some(stop)) => vec![run(stop), run(start)],
                        _ => Vec::new(),
                    },
                }
            }
        };
    }
    match action {
        Action::Start => node.children.iter().flat_map(|c| plan(c, Action::Start)).collect(),
        Action::Stop => node.children.iter().rev().flat_map(|c| plan(c, Action::Stop)).collect(),
        Action::Restart => {
            let mut steps = plan(node, Action::Stop);
            let starts = plan(node, Action::Start);
            if steps.is_empty() || starts.is_empty() {
                return Vec::new();
            }
            steps.extend(starts);
            steps
        }
    }
}

pub const PROMPT: &str = "prompt";
const PROMPT_LIMIT: usize = 65_536;

struct Run {
    command: CommandSpec,
    cwd: Option<PathBuf>,
    timeout: Duration,
    button: Option<String>,
}

fn select(prompt: &Prompt, id: &str, button: Option<&str>) -> Result<Run, String> {
    match button {
        None => Ok(Run {
            command: prompt.command.clone(),
            cwd: prompt.cwd.clone(),
            timeout: prompt.timeout,
            button: None,
        }),
        Some(name) => prompt
            .buttons
            .iter()
            .find(|button| button.name == name)
            .map(|button| Run {
                command: button.command.clone(),
                cwd: button.cwd.clone(),
                timeout: button.timeout,
                button: Some(button.name.clone()),
            })
            .ok_or_else(|| format!("service {id:?} has no button {name:?}")),
    }
}

fn run_prompt(node: &Node, prompt: &Run, text: &str) -> Result<(), String> {
    let mut command = match &prompt.command {
        CommandSpec::Shell(line) => {
            let mut command = Command::new("sh");
            command.args(["-c", line]);
            command
        }
        CommandSpec::Argv(argv) => {
            let Some((program, rest)) = argv.split_first() else {
                return Err("empty command".into());
            };
            let mut command = Command::new(program);
            command.args(rest);
            command
        }
    };
    if let Some(cwd) = &prompt.cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .env("LIVEDOT_SERVICE", &node.id)
        .env("LIVEDOT_BUTTON", prompt.button.as_deref().unwrap_or(""))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("cannot run: {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        let text = text.to_string();
        std::thread::spawn(move || {
            let _ = stdin.write_all(text.as_bytes());
        });
    }
    match wait_timeout(&mut child, prompt.timeout, true) {
        Ok(Some(status)) if status.success() => Ok(()),
        Ok(Some(status)) => Err(format!("{status}")),
        Ok(None) => Err(format!("killed after {}s timeout", prompt.timeout.as_secs())),
        Err(error) => Err(format!("wait failed: {error}")),
    }
}

pub fn prompt(shared: &Arc<Shared>, id: &str, button: Option<&str>, text: &str) -> Result<(), String> {
    let node = find(&shared.config.services, id).ok_or_else(|| format!("unknown service {id:?}"))?.clone();
    let Some(prompt) = node.prompt.as_ref() else {
        return Err(format!("service {id:?} takes no prompt"));
    };
    let prompt = select(prompt, id, button)?;
    let label = match &prompt.button {
        Some(name) => format!("{PROMPT}:{name}"),
        None => PROMPT.to_string(),
    };
    if shared.is_disabled(id) {
        return Err(format!("service {id:?} is disabled"));
    }
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("the prompt is empty".into());
    }
    if text.len() > PROMPT_LIMIT {
        return Err(format!("the prompt is longer than {PROMPT_LIMIT} bytes"));
    }
    let root = root_of(id).to_string();
    if !shared.busy.lock().unwrap().insert(root.clone()) {
        return Err(format!("service {root:?} is busy with another action"));
    }
    let shared = Arc::clone(shared);
    std::thread::spawn(move || {
        eprintln!("livedot: {label} {} accepted ({} bytes)", node.id, text.len());
        shared.changed();
        let outcome = run_prompt(&node, &prompt, &text);
        match &outcome {
            Ok(()) => eprintln!("livedot: {label} {} ok", node.id),
            Err(error) => eprintln!("livedot: {label} {} failed: {error}", node.id),
        }
        let result = ActionResult {
            action: label.clone(),
            ok: outcome.is_ok(),
            message: outcome.err().unwrap_or_else(|| "ok".to_string()),
        };
        shared.results.lock().unwrap().insert(node.id.clone(), result);
        shared.busy.lock().unwrap().remove(&root);
        shared.changed();
    });
    Ok(())
}

pub fn available(node: &Node) -> Vec<String> {
    if node.prompt.is_some() {
        return vec![PROMPT.to_string()];
    }
    Action::ALL
        .into_iter()
        .filter(|action| !plan(node, *action).is_empty())
        .map(|action| action.word().to_string())
        .collect()
}

pub fn request(shared: &Arc<Shared>, id: &str, action: &str) -> Result<(), String> {
    let node = find(&shared.config.services, id).ok_or_else(|| format!("unknown service {id:?}"))?.clone();
    let action = Action::parse(action).ok_or_else(|| format!("unknown action {action:?}: use start, stop or restart"))?;
    let steps = plan(&node, action);
    if steps.is_empty() {
        return Err(format!("service {id:?} has no way to {}", action.word()));
    }
    let root = root_of(id).to_string();
    if !shared.busy.lock().unwrap().insert(root.clone()) {
        return Err(format!("service {root:?} is busy with another action"));
    }
    let shared = Arc::clone(shared);
    std::thread::spawn(move || {
        eprintln!("livedot: control {} {} requested ({} steps)", action.word(), node.id, steps.len());
        shared.mark_action(&node, action);
        shared.changed();
        let mut failures = Vec::new();
        for step in &steps {
            match step.run() {
                Ok(()) => eprintln!("livedot: control ok {}", step.label()),
                Err(error) => {
                    eprintln!("livedot: control failed {}: {error}", step.label());
                    failures.push(format!("{} ({error})", step.label()));
                }
            }
        }
        let result = ActionResult {
            action: action.word().to_string(),
            ok: failures.is_empty(),
            message: if failures.is_empty() { format!("{} steps ok", steps.len()) } else { failures.join("; ") },
        };
        shared.results.lock().unwrap().insert(node.id.clone(), result);
        shared.busy.lock().unwrap().remove(&root);
        shared.mark_action(&node, action);
        shared.wake(&node);
        shared.changed();
    });
    Ok(())
}

fn run_item(node: &Node, command: &CommandSpec, cwd: Option<&PathBuf>, timeout: Duration, item: &str, action: &str) -> Result<(), String> {
    let spec = match command {
        CommandSpec::Shell(line) => CommandSpec::Shell(line.replace("{item}", item)),
        CommandSpec::Argv(argv) => CommandSpec::Argv(argv.iter().map(|arg| arg.replace("{item}", item)).collect()),
    };
    let mut command = command_for(&spec)?;
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command
        .env("LIVEDOT_SERVICE", &node.id)
        .env("LIVEDOT_ITEM", item)
        .env("LIVEDOT_ACTION", action)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .process_group(0)
        .spawn()
        .map_err(|e| format!("cannot run: {e}"))?;
    match wait_timeout(&mut child, timeout, true) {
        Ok(Some(status)) if status.success() => Ok(()),
        Ok(Some(status)) => Err(format!("{status}")),
        Ok(None) => Err(format!("killed after {}s timeout", timeout.as_secs())),
        Err(error) => Err(format!("wait failed: {error}")),
    }
}

pub fn item(shared: &Arc<Shared>, id: &str, item: &str, action: &str) -> Result<(), String> {
    let node = find(&shared.config.services, id).ok_or_else(|| format!("unknown service {id:?}"))?.clone();
    let button = node
        .item_actions
        .iter()
        .find(|button| button.name == action)
        .ok_or_else(|| format!("service {id:?} has no item action {action:?}"))?
        .clone();
    if shared.is_disabled(id) {
        return Err(format!("service {id:?} is disabled"));
    }
    if !valid_item_id(item) || !shared.offers(&node, item, action) {
        return Err(format!("service {id:?} does not offer {action:?} on an item {item:?} now"));
    }
    let root = root_of(id).to_string();
    if !shared.busy.lock().unwrap().insert(root.clone()) {
        return Err(format!("service {root:?} is busy with another action"));
    }
    let shared = Arc::clone(shared);
    let item = item.to_string();
    std::thread::spawn(move || {
        let label = format!("item:{}", button.name);
        eprintln!("livedot: {label} {} {item} accepted", node.id);
        shared.changed();
        let outcome = run_item(&node, &button.command, button.cwd.as_ref(), button.timeout, &item, &button.name);
        match &outcome {
            Ok(()) => eprintln!("livedot: {label} {} {item} ok", node.id),
            Err(error) => eprintln!("livedot: {label} {} {item} failed: {error}", node.id),
        }
        let result = ActionResult {
            action: label,
            ok: outcome.is_ok(),
            message: outcome.err().unwrap_or_else(|| "ok".to_string()),
        };
        shared.results.lock().unwrap().insert(node.id.clone(), result);
        shared.busy.lock().unwrap().remove(&root);
        shared.wake(&node);
        shared.changed();
    });
    Ok(())
}
