use serde_json::json;
use std::ffi::CString;
use std::path::PathBuf;
use std::process::ExitCode;

const DEFAULT_LOW_GB: f64 = 10.0;
const MAX_DISKS: usize = 16;
const ID_MAX: usize = 120;

struct Item {
    id: String,
    label: String,
    short: String,
    long: String,
}

struct Mount {
    name: String,
    id: String,
    target: String,
}

struct Args {
    show: Option<String>,
    low_gb: f64,
    state: Option<PathBuf>,
}

fn parse(args: &[String]) -> Result<Args, (u8, String)> {
    let mut show = None;
    let mut low_gb = None;
    let mut state = None;
    let mut index = 0;
    if args.first().is_some_and(|word| word == "show") {
        let item = args.get(1).ok_or_else(|| (2, "stats show needs an item".to_string()))?;
        show = Some(item.clone());
        index = 2;
    }
    while index < args.len() {
        match args[index].as_str() {
            "--low-gb" => {
                let value = args.get(index + 1).ok_or_else(|| (2, "--low-gb needs a number".to_string()))?;
                let number: f64 = value.parse().map_err(|_| (2, format!("--low-gb needs a number, not {value:?}")))?;
                if !number.is_finite() || number < 0.0 {
                    return Err((2, format!("--low-gb needs a finite number of at least 0, not {value:?}")));
                }
                low_gb = Some(number);
                index += 2;
            }
            "--state" => {
                let value = args.get(index + 1).ok_or_else(|| (2, "--state needs a path".to_string()))?;
                state = Some(PathBuf::from(value));
                index += 2;
            }
            other => return Err((2, format!("unexpected argument {other:?} for stats"))),
        }
    }
    if show.is_some() && low_gb.is_some() {
        return Err((2, "--low-gb does not go with stats show".to_string()));
    }
    Ok(Args { show, low_gb: low_gb.unwrap_or(DEFAULT_LOW_GB), state })
}

fn state_path(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local").join("state"));
    base.join("livedot").join("stats-show")
}

fn gb(bytes: u64) -> String {
    let value = bytes as f64 / 1e9;
    if value < 10.0 { format!("{value:.1} GB") } else { format!("{value:.0} GB") }
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(position) = rest.find('\\') {
        out.push_str(&rest[..position]);
        let tail = &rest[position..];
        let decoded = [("\\040", ' '), ("\\011", '\t'), ("\\012", '\n'), ("\\134", '\\')]
            .into_iter()
            .find(|(code, _)| tail.starts_with(code));
        match decoded {
            Some((code, character)) => {
                out.push(character);
                rest = &tail[code.len()..];
            }
            None => {
                out.push('\\');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn mounts() -> Vec<Mount> {
    let Ok(table) = std::fs::read_to_string("/proc/self/mounts") else {
        return Vec::new();
    };
    let mut sources: Vec<String> = Vec::new();
    let mut targets: Vec<String> = Vec::new();
    let mut ids: Vec<String> = vec!["disk:root".to_string()];
    let mut out = Vec::new();
    for line in table.lines() {
        if out.len() >= MAX_DISKS {
            break;
        }
        let mut fields = line.split_whitespace();
        let (Some(source), Some(target), Some(_fstype)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        if !source.starts_with("/dev/") || source.starts_with("/dev/loop") {
            continue;
        }
        let target = unescape(target);
        if target == "/" || target == "/boot" || target.starts_with("/boot/") {
            continue;
        }
        if sources.iter().any(|seen| seen == source) || targets.iter().any(|seen| seen == &target) {
            continue;
        }
        sources.push(source.to_string());
        targets.push(target.clone());
        let name = target.rsplit('/').find(|part| !part.is_empty()).unwrap_or("").to_string();
        let safe: String = name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
            .collect();
        let base: String = format!("disk:{safe}").chars().take(ID_MAX).collect();
        let mut id = base.clone();
        let mut counter = 2;
        while ids.contains(&id) {
            id = format!("{base}-{counter}");
            counter += 1;
        }
        ids.push(id.clone());
        out.push(Mount { name, id, target });
    }
    out
}

fn statvfs(path: &str) -> Result<(u64, u64), std::io::Error> {
    let c_path = CString::new(path).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut buffer: libc::statvfs = unsafe { std::mem::zeroed() };
    let status = unsafe { libc::statvfs(c_path.as_ptr(), &mut buffer) };
    if status != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let unit = buffer.f_frsize as u64;
    let free = (buffer.f_bavail as u64).saturating_mul(unit);
    let total = (buffer.f_blocks as u64).saturating_mul(unit);
    Ok((free, total))
}

fn meminfo() -> Option<(u64, u64, u64, u64)> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let field = |key: &str| -> Option<u64> {
        text.lines()
            .find_map(|line| line.strip_prefix(key).and_then(|rest| rest.strip_prefix(':')))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|number| number.parse::<u64>().ok())
            .map(|kb| kb.saturating_mul(1024))
    };
    Some((field("MemTotal")?, field("MemAvailable")?, field("SwapTotal")?, field("SwapFree")?))
}

fn loadavg() -> Option<(f64, u64)> {
    let text = std::fs::read_to_string("/proc/loadavg").ok()?;
    let load: f64 = text.split_whitespace().next()?.parse().ok()?;
    let cores = std::thread::available_parallelism().map(|n| n.get() as u64).unwrap_or(1);
    Some((load, cores))
}

fn other_disk_item(mount: &Mount) -> Option<Item> {
    let (free, total) = statvfs(&mount.target).ok()?;
    Some(Item {
        id: mount.id.clone(),
        label: format!("Free on {} · {}", mount.name, gb(free)),
        short: format!("{} {}", mount.name, gb(free)),
        long: format!("{} free of {} on {}", gb(free), gb(total), mount.name),
    })
}

fn ids_from_mounts() -> Vec<String> {
    let mut ids = vec!["disk:root".to_string()];
    ids.extend(mounts().into_iter().map(|mount| mount.id));
    ids.extend(["memory", "load", "swap"].map(String::from));
    ids
}

fn show(item: &str, state: PathBuf) -> Result<ExitCode, (u8, String)> {
    let ids = ids_from_mounts();
    if !ids.iter().any(|id| id == item) {
        return Err((1, format!("unknown stat {item:?}; one of: {}", ids.join(", "))));
    }
    let failed = |error: std::io::Error| (1, format!("cannot write {}: {error}", state.display()));
    if let Some(parent) = state.parent() {
        std::fs::create_dir_all(parent).map_err(failed)?;
    }
    let mut temporary = state.clone().into_os_string();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, format!("{item}\n")).map_err(failed)?;
    std::fs::rename(&temporary, &state).map_err(failed)?;
    println!("stats label now shows {item}");
    Ok(ExitCode::SUCCESS)
}

pub fn run(args: &[String]) -> Result<ExitCode, (u8, String)> {
    let parsed = parse(args)?;
    let state = state_path(parsed.state);
    if let Some(item) = parsed.show {
        return show(&item, state);
    }
    let (root_free, root_total) = match statvfs("/") {
        Ok(pair) => pair,
        Err(error) => {
            println!("{}", json!({"status": "unsure", "detail": format!("cannot read free space on /: {error}")}));
            return Ok(ExitCode::SUCCESS);
        }
    };
    let mut items = vec![Item {
        id: "disk:root".into(),
        label: format!("Free on / · {}", gb(root_free)),
        short: gb(root_free),
        long: format!("{} free of {} on /", gb(root_free), gb(root_total)),
    }];
    items.extend(mounts().iter().filter_map(other_disk_item));
    if let Some((mem_total, mem_available, swap_total, swap_free)) = meminfo() {
        items.push(Item {
            id: "memory".into(),
            label: format!("RAM available · {}", gb(mem_available)),
            short: format!("RAM {}", gb(mem_available)),
            long: format!("{} of {} memory available", gb(mem_available), gb(mem_total)),
        });
        if let Some((load, cores)) = loadavg() {
            items.push(load_item(load, cores));
        }
        items.push(swap_item(swap_total, swap_free));
    } else if let Some((load, cores)) = loadavg() {
        items.push(load_item(load, cores));
    }
    let mut unreadable = None;
    let chosen = match std::fs::read_to_string(&state) {
        Ok(text) => text.trim().to_string(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            unreadable = Some(format!(" (cannot read {}: {error})", state.display()));
            String::new()
        }
    };
    let selected = if items.iter().any(|item| item.id == chosen) { chosen } else { "disk:root".to_string() };
    let current = items.iter().find(|item| item.id == selected).unwrap_or(&items[0]);
    let low = (root_free as f64 / 1e9) < parsed.low_gb;
    let (mut status, mut detail) = if low {
        ("down", format!("low disk: {}", items[0].long))
    } else {
        ("up", current.long.clone())
    };
    if let Some(note) = unreadable {
        if status != "down" {
            status = "unsure";
        }
        detail.push_str(&note);
    }
    let listed: Vec<_> = items
        .iter()
        .map(|item| json!({"id": item.id, "label": item.label, "checked": item.id == selected, "actions": ["show"]}))
        .collect();
    println!("{}", json!({"status": status, "detail": detail, "label": current.short, "items": listed}));
    Ok(ExitCode::SUCCESS)
}

fn load_item(load: f64, cores: u64) -> Item {
    Item {
        id: "load".into(),
        label: format!("Load · {load:.1} on {cores} cores"),
        short: format!("load {load:.1}"),
        long: format!("load {load:.1} on {cores} cores"),
    }
}

fn swap_item(total: u64, free: u64) -> Item {
    if total == 0 {
        return Item { id: "swap".into(), label: "Swap · none".into(), short: "swap none".into(), long: "no swap".into() };
    }
    let used = total.saturating_sub(free);
    Item {
        id: "swap".into(),
        label: format!("Swap used · {} of {}", gb(used), gb(total)),
        short: format!("swap {}", gb(used)),
        long: format!("{} of {} swap used", gb(used), gb(total)),
    }
}
