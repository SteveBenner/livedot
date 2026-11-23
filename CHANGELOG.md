# Changelog

All notable changes to this project are recorded here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.2] - 2026-11-23

### Fixed
- The GNOME extension's switch rows work on GNOME Shell 50, where a switch row emits `toggled` whenever its state changes, including when code sets it. Each handler set the row back to the reported state, which emitted `toggled` again and sent the opposite call. A row in the **Dots** tile hid its service and showed it again in the same second, **Hide healthy dots** set the value back, and a switch tile ran its item's action twice. A row now ignores a `toggled` that matches the state the daemon reports (`extension.js`, STD-SWITCH).

## [0.12.1] - 2026-11-22

### Changed
- `dot_size` is renamed `icon_size`, with `SetIconSize`, `livedot iconsize [PERCENT]`, and an **Icon size** tile. It now scales a service's app icon in the GNOME extension's top bar (18px at 100 percent) instead of the status dot, which was the intent of the setting. The tray ignores it. A config that still has `dot_size` is refused at load, naming the field, so rename the line to `icon_size`.
- `hide_healthy` no longer removes green services from the bar. Every enabled service stays visible. A green service with an icon is drawn as its icon without the status badge, and a service without an icon keeps its dot. This applies in the tray and the top bar, in both layouts.

### Fixed
- Every status dot, badge and menu dot is back to its 0.11.0 size. In 0.12.0 the size setting scaled them (`src/dot.rs`, `extension.js`).

## [0.12.0] - 2026-11-22

### Added
- `dot_size` (percent, 50 to 200, default 100) scales the dot the tray draws and the dots the GNOME extension draws in the top bar and menus, so the dots can be made smaller or larger without editing code (`src/dot.rs`, `src/tray.rs`, `extension.js`). Config load refuses a value outside the range. The daemon holds it, publishes it with its bounds in the snapshot, and `SetDotSize` and `livedot dotsize [PERCENT]` change it live and persist it by rewriting the one line in `config.yml` under a lock, after a dated backup.
- `hide_healthy` (default false) draws no dot for a green top-level service in the tray and the GNOME top bar, in `per_service` layout, for a quieter bar; the dot returns when the service leaves green. The aggregate dot is never removed. `SetHideHealthy` and `livedot healthy hide|show` change it and persist it the same way.
- A GNOME **Dot size** Quick Settings tile with a slider and a **Hide healthy dots** switch. It is created only when the daemon publishes `dot_size`, so it does not carry its own default.

### Changed
- `Shared::new` takes the config path, and `dot::argb` and `dot::png` take a scale.

## [0.11.0] - 2026-11-06

### Added
- `one_click: true` on a top-level service with `item_actions` makes one left click on its dot in the GNOME extension run the first item's first action, with no menu. The AppIndicator extension opens the menu on a single click and calls Activate only on a double click, so the extension draws the dot itself (`extension.js`). A right click opens the normal menu, a press while the service is busy or when no item carries an action does nothing, and touch follows the left button. In `per_service` layout the extension draws a `one_click` service even with `shell_panel: false`, and the tray publishes no indicator for it. In `aggregate` layout it has no effect and the service stays in the worst-of.
- Config load refuses `one_click` on a child service, together with a `url`, and without `item_actions`, naming the service (`src/config.rs`). `livedot check` marks `[one click]`, and the snapshot carries `"one_click": true` on such a service (`src/model.rs`, `src/monitor.rs`).

## [0.10.0] - 2026-10-29

### Added
- `rollup: false` on a service keeps its own dot, colour, detail and actions but leaves it out of its parent's worst-of and detail, and a top-level one out of the aggregate dot (`src/config.rs`, `src/monitor.rs`, `src/model.rs`, STD-ROLLUP). A wrapper's dot means "is this up", and a child that reports something else, such as whether a model slot is loaded, turned it red. A wrapper with no check of its own and no child left in its rollup reads unsure. The snapshot carries `"rollup": false` on such a service.
- Dots in the bar (STD-DOTS): each top-level service can be hidden from the bar and is still checked. The daemon owns the hidden set, loads it from `$XDG_STATE_HOME/livedot/hidden` (default `~/.local/state/livedot/hidden`) at start, writes it atomically on each change, keeps lines naming services that are not in the config, and publishes `hidden: true` on those services in the snapshot (`src/monitor.rs`, `src/model.rs`). A bar of 37 dots needs a way to put some away without editing the config and restarting.
- D-Bus `SetShown(id, shown)`, refused for an unknown or child id (`src/bus.rs`), and `livedot dots [--json]`, `livedot dots --pick`, `livedot show ID` and `livedot hide ID` (`src/main.rs`). `livedot check` marks `[no rollup]`.
- The tray ends every livedot menu with **Dots…**, a zenity checklist of every top-level service by name; OK applies only what changed and Cancel nothing (`src/tray.rs`, `pick_dots`). A hidden service's tray item goes away and a shown one comes back without a restart, and the aggregate menu leaves hidden services out.
- The GNOME extension adds a **Dots** Quick Settings tile, subtitled "n of m shown", whose scrolling menu holds one switch row per top-level service sorted by name, with its icon and dot; flipping a row calls `SetShown` and the next snapshot redraws it. Panel buttons and the aggregate menu skip hidden services (`extension.js`).

## [0.9.0] - 2026-10-28

### Added
- `livedot stats [--low-gb N] [--state PATH]` prints this machine's free space on `/`, on every other block-device disk (not `/boot`, not loop devices), memory available, load and swap used, as the JSON a command check reads (`src/stats.rs`, STD-STATS). Each is a switch row with one action, `show`; `livedot stats show ITEM` records which one the label shows, in `~/.local/state/livedot/stats-show`. The status is `down` once `/` has less than `--low-gb` (default 10) decimal GB free.
- A command check's JSON may carry `label`, short text shown beside the service's own dot in the top bar (STD-LABEL). It is cut to 32 characters and replaced whenever the items are (`src/probe.rs`, `src/monitor.rs`, `src/model.rs`). The tray publishes it as the AppIndicator `XAyatanaLabel` property in `per_service` layout (`src/tray.rs`), and the GNOME extension draws it beside its panel dot when it draws panel dots.
- `vendor/ksni`: ksni 0.3.6 with `Tray::label`, published as `XAyatanaLabel` and `XAyatanaLabelGuide` with the `XAyatanaNewLabel` signal. `Cargo.toml` points ksni there through `[patch.crates-io]`; nothing else in the copy differs from the crates.io release. Neither ksni 0.3.6 nor 0.3.7 publishes a label.

### Changed
- The GNOME extension's panel button holds its dot and its label in one box, because a panel button lays out only its first child.

## [0.8.1] - 2026-10-15

### Fixed
- The daemon no longer runs on with a dead session bus connection. When emitting `Changed` fails with an I/O error it exits with status 1, so `livedot.service` restarts it (`src/bus.rs`, STD-BUS-LOST). Before, a GNOME Shell crash left it logging `cannot emit Changed: I/O error: Broken pipe` every minute while every dot froze, until a manual restart.
- The tray exits with status 1 when its `Changed` subscription ends because its own bus connection is gone, so `livedot-tray.service` restarts it (`src/tray.rs`). A daemon restart alone does not end it; the tray picks the new daemon up on its next poll.

## [0.8.0] - 2026-10-09

### Added
- Switch items: a command check's item with a boolean `checked` is a switch (STD-SWITCH). The tray draws it as a checkmark row instead of a submenu, and flipping it runs the item's first action (`src/probe.rs`, `src/model.rs`, `src/monitor.rs`, `src/tray.rs`).
- The GNOME extension adds one Quick Settings tile per service with switch items, titled by the service and subtitled "n of m on", with one switch row per item. Clicking the tile body opens it and flips nothing. A row the daemon refuses snaps back to the reported state.
- `shell_panel:` top-level key (default `true`). With `false`, the GNOME extension draws no top-bar dots, so it can run beside the tray for its tiles alone. `livedot check` prints it when false.

## [0.7.0] - 2026-10-05

### Added
- `command` checks (`check.command`): a string run with `sh -c` or a list of arguments, killed with its process group after the check timeout. Exit 0 passes; when standard output is one JSON object `{status, detail, items}`, its `status` (`up`, `down`, `unsure`, `booting`) and `detail` decide instead (`src/probe.rs`, STD-COMMAND-CHECK).
- Items: what a command check reports under `items` is held with the leaf's state, carried in the snapshot as `items`, listed in the tray menu as one submenu per item with its detail and its actions, and named in the tooltip (STD-ITEMS).
- `item_actions:` on a service with a command check: `{name, label, icon, command, cwd, timeout}`, with `{item}` replaced by the item id and `LIVEDOT_ITEM`, `LIVEDOT_ACTION` set. An item's `actions` field picks which apply. Commands come only from the config.
- D-Bus `ItemAction(id, item, action)` and `livedot item ID ITEM ACTION`. Refused unless the service reports that item with that action now; shares the one-action-at-a-time limit; wakes the check when it ends.
- Clicking the tray item of a service with no `url` and no prompt runs the first action on its first item.
- `livedot status` lists each item with its actions; `livedot check` names a service's item actions.

## [0.6.0] - 2026-10-03

### Added
- Icons on the tray menu entries: the first line carries the service's icon with its status badge (or its plain dot), and Start, Stop, Restart, **Open in browser**, **Ask…** and each button carry a freedesktop icon name.
- `icon:` on a button: a freedesktop icon name for its menu entry (default `document-edit-symbolic`).

### Changed
- The first line of a service's menu is now an ordinary entry rather than a greyed-out one, so its icon keeps its colors. Choosing it does nothing.

## [0.5.1] - 2026-10-03

### Changed
- The first line of a service's menu names the service: `logs · up · HTTP 200` instead of `up · HTTP 200`. With one tray item per service the menu did not say which service it belonged to.

## [0.5.0] - 2026-10-03

### Added
- `url:` on a service: an `http://` or `https://` address. Its menu gets **Open in browser**, which hands the address to `xdg-open`; a click on the tray item of a top-level service with a `url` and no prompt opens it too.
- `livedot open ID`.
- The snapshot carries `url`; the GNOME Shell extension shows the same menu entry.

## [0.4.0] - 2026-10-03

### Added
- Buttons on a prompt service: `prompt.buttons: [{name, label, command, cwd, timeout}]`. Each button is one more menu entry that opens a text box and runs its own command with the text on standard input and `LIVEDOT_BUTTON` set.
- `livedot prompt ID --button NAME [TEXT...]` and the D-Bus method `PromptButton(id, button, text)`.
- The snapshot lists a service's `buttons`; the GNOME Shell extension shows one text entry per button.

## [0.3.0] - 2026-10-03

### Added
- Prompt services: `prompt: {command, cwd, timeout}` on a service. Text handed to it is written to the standard input of the command. The dot is green when idle, yellow (`working`) during a run and red after a run that failed or timed out.
- `livedot tray`: a click on a prompt service's tray item, or **Ask…** in its menu, opens a one-line text box (`zenity`) and hands the text over.
- `livedot prompt ID [TEXT...]`, reading standard input when no text is given.
- D-Bus method `Prompt(id, text)` on `io.livedot.Daemon1`.
- GNOME Shell extension: a text entry in the menu of a prompt service.
- Status `working`.

## [0.2.0] - 2026-10-03

### Added
- `icon:` on a service: a path to an SVG (absolute or `~/`). `livedot tray` draws it with the status color as a corner badge, in the tray item and in menus; the GNOME Shell extension does the same in the top bar. A service without an icon keeps the plain dot.
- `livedot check` prints each icon path and reports one that cannot be read or drawn; such a service falls back to the plain dot.
- The snapshot carries `icon` for a service that has one.
- Dependency: `resvg` (no default features) to rasterize the SVG.

## [0.1.0] - 2026-10-03

### Added
- `livedot daemon`: loads a YAML config, checks each service over HTTP, by systemd unit state or by process command line, and serves the result on the session D-Bus as `io.livedot.Daemon` (`GetState`, `Control`, `Changed`).
- Status model: green `up`, red `down`, yellow `booting`, `flapping` and `unsure`.
- Wrapper services: a service with `children` shows the worst status among them and starts, stops and restarts them in order.
- Control through a systemd unit (user or system scope) or through configured commands, one action at a time per top-level service, each command under a timeout.
- `livedot tray`: StatusNotifierItem front end with a generated dot icon and nested menus; survives the daemon stopping and starting.
- GNOME Shell extension `livedot@livedot.io` for Shell 50.
- `layout: aggregate | per_service` for one worst-of dot or one dot per top-level service.
- `livedot status`, `start`, `stop`, `restart` and `check` on the command line.
- Probe limits: one in flight per service, a floor of one second, exponential slowdown on 429 and 503 with `Retry-After` honored, and the `LIVEDOT_DISABLE_PROBES=1` and `enabled: false` kill switches.
- `make install`, `make install-extension`, user units in `contrib/systemd`, examples in `examples/`.
