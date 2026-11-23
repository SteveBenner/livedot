# livedot

A red, yellow or green dot for each service you care about, with start, stop and restart in the same menu. Configured in one YAML file.

| Dot | Status | Meaning |
|---|---|---|
| green | `up` | the last check passed |
| yellow | `working` | a prompt service is running its command |
| yellow | `booting` | started or restarted through livedot and not yet passing, or its systemd unit is activating |
| yellow | `flapping` | it flipped between passing and failing several times in the last minute |
| yellow | `unsure` | not checked yet, checking disabled, the check could not run, or one failure not yet confirmed |
| red | `down` | two checks in a row failed |

## How it fits together

One binary, three roles:

- `livedot daemon` reads the config, checks every service, runs start, stop and restart, and publishes the state on the session D-Bus as `io.livedot.Daemon`.
- `livedot tray` shows the dot in the system tray (StatusNotifierItem: KDE, XFCE, and GNOME with the AppIndicator extension).
- `gnome-extension/livedot@livedot.io` shows the dot in the GNOME Shell top bar, and puts a service's switches in a Quick Settings tile.

Both front ends only render what the daemon tells them, so they always agree. Run either one, or both.

## Install

Needs Rust 1.87 or newer and a D-Bus session.

```sh
make install              # binary to ~/.local/bin, user units, a starter config if none exists
$EDITOR ~/.config/livedot/config.yml
systemctl --user enable --now livedot.service livedot-tray.service
```

For the GNOME Shell extension instead of, or beside, the tray:

```sh
make install-extension
gnome-extensions enable livedot@livedot.io    # after logging out and in again on Wayland
```

## Configure

`~/.config/livedot/config.yml` (or `$LIVEDOT_CONFIG`, or `livedot daemon --config PATH`). Unknown keys are rejected; `livedot check` validates a file and prints what it declares. Restart the daemon after editing.

A service with three sub-processes, as in `examples/shop.yml`:

```yaml
layout: aggregate
services:
  - name: shop
    children:
      - name: web
        check:
          http: http://127.0.0.1:8080/health
        control:
          unit: shop-web.service
      - name: worker
        unit: shop-worker.service
      - name: scheduler
        unit: shop-scheduler.service
```

`shop` is a wrapper: its dot is the worst of its children, **Start** starts them in order, **Stop** stops them in reverse order, and **Restart** stops all then starts all. Each child also has its own dot and its own actions. Children can have children.

### Checks

Exactly one per service.

| Key | Passes when |
|---|---|
| `http: URL` | the response status is 2xx, or is listed in `expect_status` |
| `unit: NAME` | the systemd unit is `active`; `activating` reads as booting |
| `process: TEXT` | some process has `TEXT` in its command line |
| `command: CMD` | the command exits 0, unless its output says otherwise (below) |

`interval` and `timeout` can be set per check. Redirects are not followed.

### Command checks and items

A `command` check runs a string through `sh -c`, or a list of arguments, with `LIVEDOT_SERVICE` set, and kills it after `timeout`. Exit 0 passes and anything else fails. When its standard output is one JSON object, that object can say more:

```json
{"status": "up", "detail": "2 waiting",
 "items": [{"id": "prop-9fb5", "label": "Assignment 1 due tonight", "detail": "critical", "actions": ["open", "later"]}]}
```

`status` is `up`, `down`, `unsure` or `booting` and decides for an exit 0; `detail` is the line beside the dot in the menu; `label` (optional, up to 32 characters) is text drawn beside the service's own dot in the top bar. `items` are things the service wants in front of you: each gets a submenu in the service's menu, and the tooltip names the first. What you can do to an item is declared in the config, never by the command:

```yaml
  - name: reminders
    check:
      command: ["reminders", "glance", "--json"]
      interval: 60s
      timeout: 30s
    item_actions:
      - name: open
        label: "Open"
        icon: document-open-symbolic
        command: ["reminders", "window", "open", "{item}"]
      - name: later
        label: "Later"
        command: ["reminders", "respond", "{item}", "later"]
```

`{item}` becomes the item's id, which is also in `LIVEDOT_ITEM` (and the action's name in `LIVEDOT_ACTION`). An item's `actions` picks which of these apply to it; without it, all do. Item ids are 1 to 128 letters, digits, `.`, `_`, `:` or `-`; other items are dropped, as are items past the twentieth. An action runs only on an item the service reports right now, shares the service's one-action-at-a-time limit, and refreshes the check when it ends. Clicking the tray item of a service with items and no `url` runs the first action on the first item. `livedot item reminders prop-9fb5 later` does the same from a shell.

### Switches

An item with `"checked": true` or `false` is a switch. The tray draws it as a checkmark row instead of a submenu, and the GNOME extension puts it in a Quick Settings tile named after the service, beside Wi-Fi and Do Not Disturb, with a "2 of 4 on" subtitle. Clicking the tile opens its switches and flips nothing. Flipping a switch runs the item's first action, so the command decides what a flip means by listing the opposite action first:

```json
{"status": "up", "detail": "permission banners muted",
 "items": [{"id": "mute", "label": "Mute banners", "checked": true, "actions": ["off"]},
           {"id": "sound", "label": "Play a sound", "checked": false, "actions": ["on"]}]}
```

```yaml
    item_actions:
      - name: "on"
        label: "Turn on"
        command: ["mytool", "set", "{item}", "on"]
      - name: "off"
        label: "Turn off"
        command: ["mytool", "set", "{item}", "off"]
```

The front ends keep no switch state: the action wakes the check, and its next output redraws both. To use the extension only for its tiles beside the tray, set `shell_panel: false`.

### Labels

A command check's `label` is shown as text beside the service's dot, in `per_service` layout only: the tray publishes it as the AppIndicator `XAyatanaLabel` property, which GNOME's AppIndicator extension draws next to the icon, and the GNOME extension draws it beside its own panel dot. A tray host that does not read that property shows the dot alone. ksni, the tray library, has no label, so livedot builds against a copy of ksni 0.3.6 in `vendor/ksni` that adds one.

### System stats

`livedot stats` prints free disk space, memory, load and swap in the form a command check reads, so a system dot needs no script of its own:

```yaml
  - name: system
    check:
      command: ["livedot", "stats", "--low-gb", "10"]
      interval: 30s
      timeout: 10s
    item_actions:
      - name: show
        label: "Show in the top bar"
        command: ["livedot", "stats", "show", "{item}"]
```

The label shows free space on `/` until you pick another row: free space on each other block-device disk (`/boot` and loop devices are left out), RAM available, load, or swap used. Sizes are decimal GB. `livedot stats show load` picks one from a shell; the choice is kept in `~/.local/state/livedot/stats-show` (`--state PATH` for another file). The dot turns red once `/` has had less than `--low-gb` GB free (default 10) for `fail_threshold` checks.

### Control

| Form | What runs |
|---|---|
| `control: {unit: NAME}` | `systemctl start`, `stop`, `restart` on the unit |
| `control: {start: CMD, stop: CMD, restart: CMD}` | your commands; `restart` defaults to stop then start |

A command is a string (run with `sh -c`) or a list of arguments. `cwd` and `timeout` are optional; a command that outlives its timeout is killed together with its process group. A service with no `control` and no children shows its dot without actions.

### Icons

```yaml
  - name: logs
    icon: ~/icons/logs.svg
    check:
      http: http://127.0.0.1:9000/health
```

`icon` is a path to an SVG, absolute or starting with `~/`. The service is drawn as that picture with its status color in a corner badge instead of the plain dot. Icons pay off with `layout: per_service`, where every top-level service is its own tray item. An icon that cannot be read or drawn falls back to the plain dot, and `livedot check` says so. The tray reads an icon once; restart `livedot-tray.service` after changing the file. SVG text is not drawn.

### Open in browser

```yaml
  - name: logs
    url: http://127.0.0.1:9000/
```

`url` adds **Open in browser** to the service's menu and makes `livedot open logs` work; both hand the address to `xdg-open`. Any service can have one, children included. livedot never fetches the address itself.

### Prompt services

```yaml
  - name: jarvis
    icon: ~/icons/assistant.svg
    prompt:
      command: ["file-this-note"]
      timeout: 30m
```

A prompt service checks nothing. It takes one line of text and runs its command with that text on standard input and `LIVEDOT_SERVICE` set to the service id. Click its tray item, or pick **Ask…** in its menu, to get a text box (the tray uses `zenity`); or run `livedot prompt jarvis remember the milk`. The dot is green when idle, yellow while the command runs, and red after a run that exited non-zero or outlived `timeout` (default `10m`), until the next run. One run at a time. Whatever the command wants to tell you, it tells you itself; livedot only shows the dot. A prompt service takes no check, unit, control or children.

A prompt service can carry more text boxes, each with its own command:

```yaml
    prompt:
      command: ["file-this-note"]
      buttons:
        - name: remind
          label: "Remind me…"
          icon: alarm-symbolic
          command: ["set-a-reminder"]
```

`icon` on a button is a freedesktop icon name for its menu entry.

Each button is a menu entry under **Ask…**, and `livedot prompt jarvis --button remind in 20 min tea`. Its command also gets `LIVEDOT_BUTTON`. Buttons share the service's dot and its one-run-at-a-time limit.

### Dots outside the rollup

```yaml
  - name: llm
    children:
      - name: api
        check:
          http: http://127.0.0.1:3090/health
      - name: big-model
        rollup: false
        check:
          command: ["model-slot-state"]
```

A wrapper's dot is the worst of its children. `rollup: false` keeps a service's own dot, colour, detail and actions, but its parent is computed as if it were not there, so a dot can say something other than healthy or not without turning its parent red. Here `big-model` can read red while the model is loaded and green while the slot is free, and `llm` still means the service is up. On a top-level service it leaves the aggregate dot. A wrapper with no check of its own and no child left in its rollup reads unsure.

### Dots in the bar

Every top-level service has a place in the bar until you hide it. A hidden service is still checked, still in `livedot status`, and still listed wherever dots are picked, so you can bring it back.

- **Dots…** at the end of every livedot tray menu opens a checklist of every top-level service by name (`zenity`). Tick the ones to show and press OK; Cancel changes nothing. `livedot dots --pick` opens the same window.
- The GNOME extension adds a **Dots** Quick Settings tile, subtitled "n of m shown", with one switch row per service, its icon and its dot.
- `livedot dots` lists them from a shell, `livedot hide bench` and `livedot show bench` set one.

The daemon keeps the choice in `~/.local/state/livedot/hidden` (`$XDG_STATE_HOME/livedot/hidden`), one id per line, and applies it without a restart. A line naming a service that is not in the config is kept, so commenting a service out and back in does not forget it. Only top-level services have a place in the bar; `livedot hide shop/web` is refused.

`unit: NAME` directly on a service is shorthand for a unit check and unit control together. `scope: user` (default) or `scope: system` selects `systemctl --user` or `systemctl`.

### Top-level keys

| Key | Default | |
|---|---|---|
| `layout` | `aggregate` | `aggregate`: one worst-of dot. `per_service`: one dot per top-level service |
| `one_click` | `false` | on a top-level service with `item_actions` and no `url`: in `per_service` layout one left click on its dot runs the first item's first action, and a right click opens the menu. It needs the GNOME extension, drawn even with `shell_panel: false`; the tray shows no dot for it, so without the extension the dot is not drawn at all. After the extension is first updated on Wayland, log out and back in |
| `shell_panel` | `true` | `false`: the GNOME extension draws no top-bar dots and only its Quick Settings tiles, for running it beside the tray |
| `icon_size` | `100` | percent of a service's app icon in the GNOME extension's top bar, 50 to 200. Status dots and the tray are not affected. The GNOME **Icon size** Quick Settings tile has a slider, and `livedot iconsize 150` sets it from a shell; either rewrites this line in `config.yml` after keeping a dated `config.yml.bak-<date>-livedot` |
| `hide_healthy` | `false` | `true`: a green service with an icon is drawn as its icon alone, without the status badge, in the tray and the top bar. A service without an icon keeps its dot, and no service leaves the bar. The **Icon size** tile has a switch, and `livedot healthy hide` and `livedot healthy show` set it from a shell |
| `defaults.interval` | `5s` | time between checks, at least `1s` |
| `defaults.timeout` | `3s` | per check |
| `defaults.fail_threshold` | `2` | consecutive failures before red |
| `defaults.flap_window` / `flap_threshold` | `60s` / `3` | flips inside the window that read as flapping |
| `defaults.boot_grace` | `30s` | how long a started service may fail and still read as booting |
| `defaults.max_backoff` | `30s` | ceiling for the slowdown after a 429 or 503 answer |
| `defaults.control_timeout` | `60s` | per control command |

`enabled: false` on a service stops checking it and everything under it. `LIVEDOT_DISABLE_PROBES=1` stops every check; all dots read unsure.

## Command line

```sh
livedot status            # the tree with dots
livedot status --json     # the snapshot the front ends receive
livedot restart shop     # a wrapper
livedot stop shop/web    # one child
livedot item reminders prop-9fb5 later   # one item action
livedot stats             # this machine's system stats as command-check JSON
livedot stats show memory # make the system dot's label show RAM available
livedot dots              # every top-level service by name, shown or hidden
livedot hide bench       # take it out of the bar; it is still checked
livedot show bench
livedot check --config examples/shop.yml
```

The daemon writes every status change and every control step to stderr, which is the journal under systemd: `journalctl --user -u livedot`.

If the daemon or the tray loses its session bus connection, for example when GNOME Shell crashes, it exits with status 1 and its user unit restarts it 5 seconds later. Every dot reads unsure until its next check.

## D-Bus

Session bus, name `io.livedot.Daemon`, path `/io/livedot/Daemon`, interface `io.livedot.Daemon1`.

| Member | Signature | |
|---|---|---|
| `GetState()` | `→ s` | JSON snapshot |
| `Control(id, action)` | `ss →` | `start`, `stop` or `restart`; returns once accepted |
| `Prompt(id, text)` | `ss →` | hands text to a prompt service; returns once accepted |
| `PromptButton(id, button, text)` | `sss →` | hands text to one button of a prompt service |
| `ItemAction(id, item, action)` | `sss →` | runs one item action on an item the service reports now |
| `SetShown(id, shown)` | `sb →` | shows or hides one top-level service in the bar and saves the choice |
| `Changed` signal | `s` | JSON snapshot, sent when it differs from the last one |

## License

MIT
