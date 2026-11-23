PREFIX ?= $(HOME)/.local
BINDIR := $(PREFIX)/bin
UNITDIR := $(HOME)/.config/systemd/user
CONFIGDIR := $(HOME)/.config/livedot
EXTENSION := livedot@livedot.io
EXTENSIONDIR := $(HOME)/.local/share/gnome-shell/extensions/$(EXTENSION)

.PHONY: build install install-extension uninstall

build:
	cargo build --release

install: build
	install -Dm755 target/release/livedot $(BINDIR)/livedot
	install -Dm644 contrib/systemd/livedot.service $(UNITDIR)/livedot.service
	install -Dm644 contrib/systemd/livedot-tray.service $(UNITDIR)/livedot-tray.service
	test -e $(CONFIGDIR)/config.yml || install -Dm644 examples/config.yml $(CONFIGDIR)/config.yml
	systemctl --user daemon-reload

install-extension:
	install -Dm644 gnome-extension/$(EXTENSION)/metadata.json $(EXTENSIONDIR)/metadata.json
	install -Dm644 gnome-extension/$(EXTENSION)/extension.js $(EXTENSIONDIR)/extension.js

uninstall:
	-systemctl --user disable --now livedot-tray.service livedot.service
	rm -f $(BINDIR)/livedot $(UNITDIR)/livedot.service $(UNITDIR)/livedot-tray.service
	rm -f $(EXTENSIONDIR)/metadata.json $(EXTENSIONDIR)/extension.js
	systemctl --user daemon-reload
