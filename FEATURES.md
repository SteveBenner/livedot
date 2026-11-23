# FEATURES — livedot

**The registry of what livedot can do, with build and deployment status.**

livedot shows a red, yellow or green dot for each configured service and offers start, stop and restart from the same menu.

| | |
|---|---|
| **Registry version** | 0.12.1 |
| **Last audited** | 2026-10-29 |
| **Coverage** | Complete: every surface in `README.md` and `specs/app.yml`. |
| **Running instance** | The author's GNOME Wayland desktop, with `livedot.service` and `livedot-tray.service` in per-service layout watching a few dozen services: HTTP checks, user units, command checks with item actions and switch items, a prompt service with a button, and a `livedot stats` check. The GNOME extension runs with `shell_panel: false` for its Quick Settings tile. 🟢 rows are in use there; the rest were exercised from `target/debug/livedot` against throwaway services. |
| **Feature count** | 31 catalogued |

Build: ✅ Shipped · 🟡 Partial · ⚪ Planned. Deploy: 🟢 Live · 🔵 Built, not enabled.

| Feature | Build | Deploy | Blocker |
|---|---|---|---|
| YAML config with validation and `livedot check` | ✅ | 🟢 | — |
| HTTP check with `expect_status` | ✅ | 🟢 | — |
| systemd unit check, user and system scope | ✅ | 🟢 | system scope never run |
| Process command-line check | ✅ | 🔵 | not used by the installed config |
| Command check with a JSON status, detail and items | ✅ | 🟢 | one service; the exit-code-only form and the timeout kill were run against throwaway commands on a private bus |
| Item actions: config-declared commands on reported items, `livedot item`, D-Bus `ItemAction` | ✅ | 🟢 | run through `livedot item` on a private bus; tray only, not the extension; the live menu entries were read back over D-Bus, never clicked by a person |
| Service icons: an SVG with the status as a corner badge | ✅ | 🟢 | tray only; the extension side has never run in a Shell |
| Status model: up, down, booting, flapping, unsure | ✅ | 🟢 | — |
| Wrapper services with ordered start, stop, restart | ✅ | 🟢 | — |
| Control by systemd unit | ✅ | 🟢 | user scope run against a throwaway unit; system scope never run |
| Prompt services: text to a command, by `livedot prompt`, D-Bus `Prompt` and the tray text box | ✅ | 🟢 | run through `livedot prompt`; the tray text box was never clicked by a person; the extension entry has never run in a Shell; the timeout kill was not exercised |
| Buttons on a prompt service | ✅ | 🟢 | run through `livedot prompt --button`; the tray menu entry was never clicked by a person; the extension entry has never run in a Shell |
| `url:` with Open in browser and `livedot open` | ✅ | 🟢 | run through `livedot open`; the menu entry was never clicked by a person; the extension entry has never run in a Shell |
| Icons on tray menu entries | ✅ | 🟢 | tray only, not the extension; the entries were read back over D-Bus, the rendered menu was not looked at by a person |
| Control by configured commands, with timeout | ✅ | 🟢 | in use on several services; no action has been run through it there |
| D-Bus service `io.livedot.Daemon` with `GetState`, `Control`, `Changed` | ✅ | 🟢 | — |
| Exit on a lost session bus so the units restart the daemon and tray | ✅ | 🟢 | run against a killed private bus and a restarted daemon; a live GNOME Shell crash has not recurred since |
| Tray front end, aggregate and per-service layout | ✅ | 🟢 | run on GNOME only |
| GNOME Shell extension | 🟡 | 🟢 | enabled 2026-10-09 with `shell_panel: false`, ACTIVE with no errors; its top-bar dots, prompt entries and Open in browser have not been looked at by a person |
| Switch items: checkmark rows in the tray, first action on a flip | ✅ | 🟢 | one service; a live tray click on the sound row wrote the file and redrew the row, then `livedot item` switched it back |
| Quick Settings tiles for switch items, `shell_panel` | ✅ | 🟢 | a switch tile was seen and flipped by a person on 2026-10-09 |
| Labels: a command check's `label` beside the service's dot, as the tray's `XAyatanaLabel` and in the extension's panel button | ✅ | 🟢 | one service; the live tray item's `XAyatanaLabel` read `23 GB` over D-Bus and a private-bus run saw it change with `XAyatanaNewLabel`; nobody has yet seen GNOME draw it in the top bar; the extension side has never run in a Shell |
| `one_click: true`: one left click on the service's dot in the GNOME extension runs its first item's first action | ✅ | 🔵 | proven only on a nested gnome-shell with synthesized presses on a private bus; no physical click, nothing installed, not in the live Shell |
| `rollup: false`: a dot kept out of its parent's worst-of and the aggregate | ✅ | 🟢 | one child (a model slot that reads red when loaded, yellow loading, green free); on a private bus its wrapper read `up · all up` beside the red child |
| `icon_size` and `hide_healthy`: top-bar app icon size and badge-free healthy icons, `SetIconSize`, `SetHideHealthy`, `livedot iconsize`, `livedot healthy` | ✅ | 🔵 | range refusal, refusal of the old `dot_size` key, persistence into a scratch `config.yml`, and the tray pixmaps (green logo unbadged, green plain dot unchanged, red logo badged, every item kept) were run on a private bus; the GNOME **Icon size** tile, the scaled top-bar logo and the hidden badge were syntax-checked only and have not been seen in a Shell |
| Dots in the bar: hidden set in the daemon, `SetShown`, `livedot dots`, `show`, `hide` | ✅ | 🟢 | run on a private bus: hide, show, persistence across a restart, child refusal, an unknown line kept |
| Tray **Dots…** checklist (`livedot dots --pick`) | ✅ | 🟢 | run with a stand-in zenity on a private bus; the real window has not been opened by a person; unticking a shown service and a missing zenity were not exercised |
| GNOME **Dots** Quick Settings tile | ✅ | 🔵 | syntax-checked only; a Wayland Shell loads extension code at login, so the tile appears after the next login and has not been seen by a person |
| `livedot stats` and `livedot stats show`: disks, memory, load, swap as switch rows, red under `--low-gb` on `/` | ✅ | 🟢 | one service, polled every 30s; the red path was run with `--low-gb 1000` from the shell, never live; picking a row was run through `livedot item` on a private bus, the live menu rows were never clicked by a person |
| Vendored ksni 0.3.6 with `XAyatanaLabel` (`vendor/ksni`) | ✅ | 🟢 | the copy differs from the crates.io release only in `src/lib.rs`, `src/dbus_interface.rs` and `src/service.rs`; upstream fixes in later ksni releases are not picked up until the copy is refreshed |
| Command line: `status`, `start`, `stop`, `restart` | ✅ | 🟢 | — |
| Probe limits and kill switches | ✅ | 🟢 | — |
| `make install`, user units | ✅ | 🟢 | `make uninstall` never run |
