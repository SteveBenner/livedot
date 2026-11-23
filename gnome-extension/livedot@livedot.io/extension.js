import Clutter from 'gi://Clutter';
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import GObject from 'gi://GObject';
import St from 'gi://St';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as QuickSettings from 'resource:///org/gnome/shell/ui/quickSettings.js';
import {Slider} from 'resource:///org/gnome/shell/ui/slider.js';

const BUS_NAME = 'io.livedot.Daemon';
const OBJECT_PATH = '/io/livedot/Daemon';
const INTERFACE = 'io.livedot.Daemon1';
const CALL_TIMEOUT_MS = 5000;
const COLORS = {green: '#33d17a', yellow: '#f6d32d', red: '#e01b24'};
const PANEL_DOT = 12;
const MENU_DOT = 10;
const PANEL_LOGO = 18;
const BADGE_DOT = 8;
const NOT_RUNNING = 'the livedot daemon is not running';
const DEFAULT_ICON = 'emblem-system-symbolic';
const MIN_LOGO_PX = 8;
const SIZE_STEP = 5;
const RESET_PERCENT = 100;
const POSITIONS = [['left', 'Left'], ['center', 'Center'], ['right', 'Right']];
const DEFAULT_POSITION = 'right';

function panelPosition(snapshot) {
    return POSITIONS.some(([position]) => position === snapshot.panel_position)
        ? snapshot.panel_position : DEFAULT_POSITION;
}

function dotStyle(color, size) {
    const fill = COLORS[color] ?? COLORS.yellow;
    return `background-color: ${fill}; border-radius: ${size}px; width: ${size}px; height: ${size}px; margin: 0 4px;`;
}

function scaled(base, scale) {
    return Math.max(MIN_LOGO_PX, Math.round(base * scale));
}

function makeDot(color, size) {
    return new St.Widget({
        style: dotStyle(color, size),
        x_align: Clutter.ActorAlign.CENTER,
        y_align: Clutter.ActorAlign.CENTER,
    });
}

function switchItems(service) {
    return (service.items ?? []).filter(item => typeof item.checked === 'boolean');
}

function serviceIcon(service) {
    return service.icon ? new Gio.FileIcon({file: Gio.File.new_for_path(service.icon)}) : null;
}

function flatten(services) {
    const out = [];
    for (const service of services) {
        out.push(service);
        out.push(...flatten(service.children ?? []));
    }
    return out;
}

function capitalize(word) {
    return word.charAt(0).toUpperCase() + word.slice(1);
}

const LivedotButton = GObject.registerClass(
class LivedotButton extends PanelMenu.Button {
    _init(title, onControl, onItem) {
        super._init(0.5, title, false);
        this._onControl = onControl;
        this._onItem = onItem;
        this._oneClick = false;
        this._service = null;
        this._scale = 1;
        this._color = 'yellow';
        this._hideHealthy = false;
        this._oneClickGesture = new Clutter.ClickGesture();
        this._oneClickGesture.set_recognize_on_press(true);
        this._oneClickGesture.set_enabled(false);
        this._oneClickGesture.connect('recognize', () => {
            const button = this._oneClickGesture.get_button();
            if (button === Clutter.BUTTON_PRIMARY || button === 0)
                this._press();
            else
                this.menu?.toggle();
        });
        this.add_action(this._oneClickGesture);
        this._iconPath = null;
        this._logo = null;
        this._stack = new St.Widget({
            layout_manager: new Clutter.BinLayout(),
            y_align: Clutter.ActorAlign.CENTER,
        });
        this._dot = makeDot('yellow', PANEL_DOT);
        this._stack.add_child(this._dot);
        this._box = new St.BoxLayout({y_align: Clutter.ActorAlign.CENTER});
        this._box.add_child(this._stack);
        this._label = new St.Label({text: '', y_align: Clutter.ActorAlign.CENTER, visible: false});
        this._box.add_child(this._label);
        this.add_child(this._box);
    }

    _setOneClick(on) {
        this._oneClick = on;
        this._clickGesture.set_enabled(!on);
        this._oneClickGesture.set_enabled(on);
    }

    _press() {
        const service = this._service;
        if (!service || service.busy)
            return;
        for (const item of service.items ?? []) {
            if (item.actions.length > 0) {
                this._onItem(service.id, item.id, item.actions[0].name);
                return;
            }
        }
    }

    setScale(scale) {
        this._scale = scale;
        this._logo?.set_icon_size(scaled(PANEL_LOGO, scale));
    }

    setHideHealthy(hide) {
        this._hideHealthy = hide;
        this._syncBadge();
    }

    _syncBadge() {
        this._dot.visible = !(this._logo && this._color === 'green' && this._hideHealthy);
    }

    setLabel(text) {
        const shown = typeof text === 'string' && text !== '';
        this._label.set_text(shown ? text : '');
        this._label.visible = shown;
    }

    setColor(color) {
        this._color = color;
        this._dot.set_style(dotStyle(color, this._logo ? BADGE_DOT : PANEL_DOT));
        this._syncBadge();
    }

    setIcon(path) {
        const wanted = path ?? null;
        if (wanted === this._iconPath)
            return;
        this._iconPath = wanted;
        this._logo?.destroy();
        this._logo = null;
        if (wanted) {
            this._logo = new St.Icon({
                gicon: new Gio.FileIcon({file: Gio.File.new_for_path(wanted)}),
                icon_size: scaled(PANEL_LOGO, this._scale),
            });
            this._stack.insert_child_at_index(this._logo, 0);
        }
        this._dot.x_expand = this._logo !== null;
        this._dot.y_expand = this._logo !== null;
        this._dot.x_align = this._logo ? Clutter.ActorAlign.END : Clutter.ActorAlign.CENTER;
        this._dot.y_align = this._logo ? Clutter.ActorAlign.END : Clutter.ActorAlign.CENTER;
        this._syncBadge();
    }

    showProblem(text) {
        this._service = null;
        this.setLabel(null);
        this.setColor('yellow');
        this.menu.removeAll();
        this._note(this.menu, text);
    }

    showAggregate(snapshot) {
        this._setOneClick(false);
        this._service = null;
        this.setLabel(null);
        this.setIcon(null);
        this.setColor(snapshot.color);
        this.menu.removeAll();
        for (const service of snapshot.services.filter(service => !service.hidden)) {
            const item = new PopupMenu.PopupSubMenuMenuItem(`${service.name} · ${service.status}`);
            item.insert_child_at_index(makeDot(service.color, MENU_DOT), 1);
            this._fill(item.menu, service, '');
            this.menu.addMenuItem(item);
        }
    }

    showService(service) {
        this._setOneClick(service.one_click === true);
        this._service = this._oneClick ? service : null;
        this.setLabel(service.label);
        this.setIcon(service.icon);
        this.setColor(service.color);
        this.menu.removeAll();
        this._fill(this.menu, service, '');
    }

    _entry(menu, service, button) {
        const item = new PopupMenu.PopupBaseMenuItem({reactive: false, can_focus: false});
        const entry = new St.Entry({
            hint_text: button ? button.label : `Tell ${service.name}…`,
            x_expand: true,
            can_focus: true,
            reactive: !service.busy,
        });
        entry.clutter_text.connect('activate', () => {
            const text = entry.get_text().trim();
            if (text === '')
                return;
            entry.set_text('');
            this.menu.close();
            this._onControl(service.id, 'prompt', text, button?.name ?? null);
        });
        item.add_child(entry);
        menu.addMenuItem(item);
    }

    _note(menu, text) {
        const item = new PopupMenu.PopupMenuItem(text, {reactive: false});
        menu.addMenuItem(item);
        return item;
    }

    _fill(menu, service, suffix) {
        if (suffix === '')
            this._note(menu, `${service.name} · ${service.status} · ${service.detail}`);
        if (service.last_action && !service.last_action.ok)
            this._note(menu, `last ${service.last_action.action} failed: ${service.last_action.message}`);
        if (service.busy && suffix === '')
            this._note(menu, 'action in progress');
        if (service.url)
            menu.addAction(`Open in browser${suffix}`, () => Gio.AppInfo.launch_default_for_uri(service.url, null));
        for (const action of service.actions) {
            if (action === 'prompt') {
                this._entry(menu, service, null);
                for (const button of service.buttons ?? [])
                    this._entry(menu, service, button);
                continue;
            }
            const item = menu.addAction(`${capitalize(action)}${suffix}`, () => this._onControl(service.id, action));
            item.setSensitive(!service.busy);
        }
        for (const child of service.children) {
            menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
            const header = this._note(menu, `${child.name} · ${child.status} · ${child.detail}`);
            header.insert_child_at_index(makeDot(child.color, MENU_DOT), 1);
            this._fill(menu, child, ` ${child.id.split('/').slice(1).join('/')}`);
        }
    }
});

const SwitchTile = GObject.registerClass(
class SwitchTile extends QuickSettings.QuickMenuToggle {
    _init(onItem) {
        super._init({title: '', iconName: DEFAULT_ICON, toggleMode: false});
        this._onItem = onItem;
        this._rowsKey = null;
        this._checked = false;
        this.connect('clicked', () => {
            this.menu.open();
            this.checked = this._checked;
        });
    }

    update(service) {
        const items = switchItems(service);
        const on = items.filter(item => item.checked).length;
        const gicon = serviceIcon(service);
        this._checked = on > 0;
        this.title = service.name;
        this.subtitle = `${on} of ${items.length} on`;
        this.checked = this._checked;
        if (gicon)
            this.gicon = gicon;
        else
            this.iconName = DEFAULT_ICON;
        this.menu.setHeader(gicon ?? DEFAULT_ICON, service.name, service.detail);
        const key = JSON.stringify([service.busy, items.map(item => [item.id, item.label, item.checked])]);
        if (key === this._rowsKey)
            return;
        this._rowsKey = key;
        this.menu.removeAll();
        for (const item of items) {
            const row = new PopupMenu.PopupSwitchMenuItem(item.label, item.checked);
            row.setSensitive(!service.busy && item.actions.length > 0);
            row.connect('toggled', (_row, state) => {
                if (state === item.checked)
                    return;
                if (item.actions.length > 0)
                    this._onItem(service.id, item.id, item.actions[0].name);
                row.setToggleState(item.checked);
            });
            this.menu.addMenuItem(row);
        }
    }
});

const DotsTile = GObject.registerClass(
class DotsTile extends QuickSettings.QuickMenuToggle {
    _init(onShown) {
        super._init({title: 'Dots', iconName: 'view-list-symbolic', toggleMode: false});
        this._onShown = onShown;
        this._rowsKey = null;
        this._checked = false;
        this._rows = new PopupMenu.PopupMenuSection();
        const scroll = new St.ScrollView({
            hscrollbar_policy: St.PolicyType.NEVER,
            vscrollbar_policy: St.PolicyType.AUTOMATIC,
            style: 'max-height: 28em;',
        });
        scroll.set_child(this._rows.box);
        this._rows.actor = scroll;
        this.menu.addMenuItem(this._rows);
        this.connect('clicked', () => {
            this.menu.open();
            this.checked = this._checked;
        });
    }

    update(services) {
        const shown = services.filter(service => !service.hidden).length;
        const subtitle = `${shown} of ${services.length} shown`;
        this._checked = shown > 0;
        this.subtitle = subtitle;
        this.checked = this._checked;
        this.menu.setHeader('view-list-symbolic', 'Dots', subtitle);
        const sorted = [...services].sort((a, b) => a.name.localeCompare(b.name, undefined, {sensitivity: 'base'}));
        const key = JSON.stringify(sorted.map(service => [service.id, service.name, service.hidden === true, service.color, service.icon ?? null]));
        if (key === this._rowsKey)
            return;
        this._rowsKey = key;
        this._rows.removeAll();
        for (const service of sorted) {
            const row = new PopupMenu.PopupSwitchMenuItem(service.name, !service.hidden);
            row.insert_child_at_index(makeDot(service.color, MENU_DOT), 1);
            const gicon = serviceIcon(service);
            if (gicon)
                row.insert_child_at_index(new St.Icon({gicon, icon_size: 16}), 1);
            row.connect('toggled', (_item, state) => {
                if (state === !service.hidden)
                    return;
                this._onShown(service.id, state);
                row.setToggleState(!service.hidden);
            });
            this._rows.addMenuItem(row);
        }
    }
});

const IconSizeTile = GObject.registerClass(
class IconSizeTile extends QuickSettings.QuickMenuToggle {
    _init(onSize, onHide, onPosition) {
        super._init({title: 'Icon size', iconName: 'format-text-size-symbolic', toggleMode: false});
        this._onSize = onSize;
        this._onHide = onHide;
        this._onPosition = onPosition;
        this._position = DEFAULT_POSITION;
        this._percent = null;
        this._min = 0;
        this._max = 0;
        this._hide = false;
        this._dragging = false;
        this._syncing = false;
        this._slider = new Slider(0);
        this._slider.x_expand = true;
        this._slider.connect('drag-begin', () => {
            this._dragging = true;
            return Clutter.EVENT_PROPAGATE;
        });
        this._slider.connect('drag-end', () => {
            this._dragging = false;
            this._commit();
            return Clutter.EVENT_PROPAGATE;
        });
        this._slider.connect('notify::value', () => {
            if (this._syncing)
                return;
            if (this._dragging)
                this._showPending();
            else
                this._commit();
        });
        const sliderItem = new PopupMenu.PopupBaseMenuItem({activate: false});
        sliderItem.add_child(this._slider);
        this.menu.addMenuItem(sliderItem);
        this.menu.addAction(`Reset to ${RESET_PERCENT}%`, () => this._onSize(RESET_PERCENT));
        this._switch = new PopupMenu.PopupSwitchMenuItem('Hide healthy dots', false);
        this._switch.connect('toggled', (_item, state) => {
            if (state === this._hide)
                return;
            this._onHide(state);
            this._switch.setToggleState(this._hide);
        });
        this.menu.addMenuItem(this._switch);
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem('Position in the bar'));
        this._positions = new Map();
        for (const [position, label] of POSITIONS) {
            const item = this.menu.addAction(label, () => {
                if (position !== this._position)
                    this._onPosition(position);
            });
            item.setOrnament(PopupMenu.Ornament.NO_DOT);
            this._positions.set(position, item);
        }
        this.connect('clicked', () => this.menu.open());
    }

    _pending() {
        const raw = this._min + this._slider.value * (this._max - this._min);
        const snapped = Math.round(raw / SIZE_STEP) * SIZE_STEP;
        return Math.min(this._max, Math.max(this._min, snapped));
    }

    _subtitle(percent) {
        return this._hide ? `${percent}%, healthy hidden` : `${percent}%`;
    }

    _showPending() {
        this.subtitle = this._subtitle(this._pending());
    }

    _commit() {
        const percent = this._pending();
        this.subtitle = this._subtitle(percent);
        if (percent !== this._percent)
            this._onSize(percent);
    }

    update(snapshot) {
        this._percent = snapshot.icon_size;
        this._min = snapshot.icon_size_min;
        this._max = snapshot.icon_size_max;
        this._hide = snapshot.hide_healthy === true;
        this.subtitle = this._subtitle(this._percent);
        this._switch.setToggleState(this._hide);
        this._position = panelPosition(snapshot);
        for (const [position, item] of this._positions)
            item.setOrnament(position === this._position ? PopupMenu.Ornament.DOT : PopupMenu.Ornament.NO_DOT);
        this.menu.setHeader('format-text-size-symbolic', 'Icon size', this.subtitle);
        if (!this._dragging) {
            this._syncing = true;
            this._slider.value = (this._percent - this._min) / (this._max - this._min);
            this._syncing = false;
        }
    }
});

export default class LivedotExtension extends Extension {
    enable() {
        this._buttons = new Map();
        this._tiles = new Map();
        this._dotsTile = null;
        this._sizeTile = null;
        this._iconScale = 1;
        this._shellPanel = true;
        this._panelPosition = DEFAULT_POSITION;
        this._indicator = new QuickSettings.SystemIndicator();
        Main.panel.statusArea.quickSettings.addExternalIndicator(this._indicator);
        this._cancellable = new Gio.Cancellable();
        this._signalId = Gio.DBus.session.signal_subscribe(
            BUS_NAME, INTERFACE, 'Changed', OBJECT_PATH, null, Gio.DBusSignalFlags.NONE,
            (_connection, _sender, _path, _iface, _signal, parameters) => {
                const [json] = parameters.deepUnpack();
                this._apply(json);
            });
        this._watchId = Gio.bus_watch_name(
            Gio.BusType.SESSION, BUS_NAME, Gio.BusNameWatcherFlags.NONE,
            () => this._refresh(),
            () => this._problem(NOT_RUNNING));
    }

    disable() {
        if (this._signalId) {
            Gio.DBus.session.signal_unsubscribe(this._signalId);
            this._signalId = null;
        }
        if (this._watchId) {
            Gio.bus_unwatch_name(this._watchId);
            this._watchId = null;
        }
        this._cancellable?.cancel();
        this._cancellable = null;
        for (const button of this._buttons?.values() ?? [])
            button.destroy();
        this._buttons = null;
        for (const tile of this._tiles?.values() ?? [])
            tile.destroy();
        this._tiles = null;
        this._dotsTile?.destroy();
        this._dotsTile = null;
        this._sizeTile?.destroy();
        this._sizeTile = null;
        this._indicator?.quickSettingsItems.splice(0);
        this._indicator?.destroy();
        this._indicator = null;
    }

    _dropTiles() {
        for (const tile of this._tiles?.values() ?? [])
            tile.destroy();
        this._tiles?.clear();
        this._dotsTile?.destroy();
        this._dotsTile = null;
        this._sizeTile?.destroy();
        this._sizeTile = null;
        this._indicator?.quickSettingsItems.splice(0);
    }

    _syncTiles(snapshot) {
        if (!this._tiles)
            return;
        if (!this._dotsTile) {
            this._dotsTile = new DotsTile((id, shown) => this._setShown(id, shown));
            this._indicator.quickSettingsItems.push(this._dotsTile);
            Main.panel.statusArea.quickSettings.menu.addItem(this._dotsTile, 1);
        }
        this._dotsTile.update(snapshot.services);
        this._syncSizeTile(snapshot);
        const services = new Map();
        for (const service of flatten(snapshot.services)) {
            if (switchItems(service).length > 0)
                services.set(service.id, service);
        }
        for (const [id, tile] of this._tiles) {
            if (!services.has(id)) {
                tile.destroy();
                this._tiles.delete(id);
                const index = this._indicator.quickSettingsItems.indexOf(tile);
                if (index >= 0)
                    this._indicator.quickSettingsItems.splice(index, 1);
            }
        }
        for (const [id, service] of services) {
            let tile = this._tiles.get(id);
            if (!tile) {
                tile = new SwitchTile((serviceId, item, action) => this._itemAction(serviceId, item, action));
                this._tiles.set(id, tile);
                this._indicator.quickSettingsItems.push(tile);
                Main.panel.statusArea.quickSettings.menu.addItem(tile, 1);
            }
            tile.update(service);
        }
    }

    _syncSizeTile(snapshot) {
        const sized = typeof snapshot.icon_size === 'number' &&
            typeof snapshot.icon_size_min === 'number' && typeof snapshot.icon_size_max === 'number';
        if (!sized) {
            if (this._sizeTile) {
                this._sizeTile.destroy();
                const index = this._indicator.quickSettingsItems.indexOf(this._sizeTile);
                if (index >= 0)
                    this._indicator.quickSettingsItems.splice(index, 1);
                this._sizeTile = null;
            }
            return;
        }
        if (!this._sizeTile) {
            this._sizeTile = new IconSizeTile(percent => this._setIconSize(percent), hide => this._setHideHealthy(hide),
                position => this._setPanelPosition(position));
            this._indicator.quickSettingsItems.push(this._sizeTile);
            Main.panel.statusArea.quickSettings.menu.addItem(this._sizeTile, 1);
        }
        this._sizeTile.update(snapshot);
    }

    _setIconSize(percent) {
        this._call('SetIconSize', new GLib.Variant('(u)', [percent]), `icon size ${percent}% refused`);
    }

    _setHideHealthy(hide) {
        this._call('SetHideHealthy', new GLib.Variant('(b)', [hide]), `${hide ? 'hide' : 'show'} healthy dots refused`);
    }

    _setPanelPosition(position) {
        this._call('SetPanelPosition', new GLib.Variant('(s)', [position]), `bar position ${position} refused`);
    }

    _call(method, args, failure) {
        Gio.DBus.session.call(
            BUS_NAME, OBJECT_PATH, INTERFACE, method, args, null,
            Gio.DBusCallFlags.NONE, CALL_TIMEOUT_MS, this._cancellable,
            (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch (error) {
                    if (!error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        Main.notifyError('livedot', `${failure}: ${error.message}`);
                }
            });
    }

    _itemAction(id, item, action) {
        Gio.DBus.session.call(
            BUS_NAME, OBJECT_PATH, INTERFACE, 'ItemAction', new GLib.Variant('(sss)', [id, item, action]), null,
            Gio.DBusCallFlags.NONE, CALL_TIMEOUT_MS, this._cancellable,
            (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch (error) {
                    if (!error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        Main.notifyError('livedot', `${action} ${item} on ${id} refused: ${error.message}`);
                }
            });
    }

    _setShown(id, shown) {
        Gio.DBus.session.call(
            BUS_NAME, OBJECT_PATH, INTERFACE, 'SetShown', new GLib.Variant('(sb)', [id, shown]), null,
            Gio.DBusCallFlags.NONE, CALL_TIMEOUT_MS, this._cancellable,
            (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch (error) {
                    if (!error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        Main.notifyError('livedot', `${shown ? 'show' : 'hide'} ${id} refused: ${error.message}`);
                }
            });
    }

    _refresh() {
        Gio.DBus.session.call(
            BUS_NAME, OBJECT_PATH, INTERFACE, 'GetState', null, new GLib.VariantType('(s)'),
            Gio.DBusCallFlags.NONE, CALL_TIMEOUT_MS, this._cancellable,
            (connection, result) => {
                try {
                    const [json] = connection.call_finish(result).deepUnpack();
                    this._apply(json);
                } catch (error) {
                    if (!error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        this._problem(`the livedot daemon is not answering: ${error.message}`);
                }
            });
    }

    _control(id, action, text = null, button = null) {
        const prompt = action === 'prompt';
        const method = prompt ? (button ? 'PromptButton' : 'Prompt') : 'Control';
        const args = button
            ? new GLib.Variant('(sss)', [id, button, text])
            : new GLib.Variant('(ss)', [id, prompt ? text : action]);
        Gio.DBus.session.call(
            BUS_NAME, OBJECT_PATH, INTERFACE, method, args, null,
            Gio.DBusCallFlags.NONE, CALL_TIMEOUT_MS, this._cancellable,
            (connection, result) => {
                try {
                    connection.call_finish(result);
                } catch (error) {
                    if (!error.matches(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                        Main.notifyError('livedot', `${action} ${id} refused: ${error.message}`);
                }
            });
    }

    _sync(keys) {
        if (!this._buttons)
            return false;
        for (const [key, button] of this._buttons) {
            if (!keys.includes(key)) {
                button.destroy();
                this._buttons.delete(key);
            }
        }
        for (const key of keys) {
            if (this._buttons.has(key))
                continue;
            const title = key === '' ? 'livedot' : `livedot: ${key}`;
            const button = new LivedotButton(title, (id, action, text, button) => this._control(id, action, text, button),
                (id, item, action) => this._itemAction(id, item, action));
            Main.panel.addToStatusArea(`${this.uuid}-${key}`, button,
                this._panelPosition === DEFAULT_POSITION ? 0 : -1, this._panelPosition);
            button.setScale(this._iconScale);
            this._buttons.set(key, button);
        }
        return true;
    }

    _problem(text) {
        if (!this._buttons)
            return;
        this._dropTiles();
        if (!this._shellPanel)
            return;
        if (this._buttons.size === 0)
            this._sync(['']);
        for (const button of this._buttons.values())
            button.showProblem(text);
    }

    _apply(json) {
        let snapshot;
        try {
            snapshot = JSON.parse(json);
        } catch (error) {
            this._problem(`unreadable daemon state: ${error.message}`);
            return;
        }
        this._shellPanel = snapshot.shell_panel !== false;
        const position = panelPosition(snapshot);
        if (position !== this._panelPosition) {
            this._panelPosition = position;
            for (const button of this._buttons?.values() ?? [])
                button.destroy();
            this._buttons?.clear();
        }
        if (typeof snapshot.icon_size === 'number')
            this._iconScale = snapshot.icon_size / 100;
        for (const button of this._buttons?.values() ?? []) {
            button.setScale(this._iconScale);
            button.setHideHealthy(snapshot.hide_healthy === true);
        }
        this._syncTiles(snapshot);
        const perService = snapshot.layout === 'per_service';
        if (!this._shellPanel && !perService) {
            this._sync([]);
            return;
        }
        const visible = snapshot.services.filter(service =>
            !service.hidden && (this._shellPanel || service.one_click === true));
        const keys = perService ? visible.map(service => service.id) : [''];
        if (!this._sync(keys))
            return;
        if (!perService) {
            this._buttons.get('').showAggregate(snapshot);
            return;
        }
        for (const service of visible)
            this._buttons.get(service.id).showService(service);
    }
}
