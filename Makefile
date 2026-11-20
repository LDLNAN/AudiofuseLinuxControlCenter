PREFIX ?= /usr
BINDIR ?= $(PREFIX)/bin
UDEVDIR ?= $(or $(shell pkg-config --variable=udevdir udev),$(PREFIX)/lib/udev)/rules.d
APPDIR ?= $(PREFIX)/share/applications
ICONDIR ?= $(PREFIX)/share/icons/hicolor/scalable/apps

.PHONY: all build test clean install uninstall udev-reload appimage deb screenshot

all: build

build:
	cargo build --locked --release --workspace

test:
	cargo test --workspace
	bash tests/helper-payload.sh
	bash tests/dkms-hook.sh
	$(MAKE) -C kmod test-policy

clean:
	cargo clean

HELPER_DIR = $(DESTDIR)$(PREFIX)/src/audiofuse8pre-ctl-0.2.1
HELPER_FILES = kmod/audiofuse8pre_ctl.c kmod/audiofuse8pre_ctl.h kmod/audiofuse8pre_policy.h kmod/Makefile kmod/dkms.conf

.PHONY: install-helper
install-helper:
	install -Dm644 packaging/audiofuse8pre-ctl.conf "$(DESTDIR)$(PREFIX)/lib/modules-load.d/audiofuse8pre-ctl.conf"
	install -Dm755 packaging/audiofuse8pre-dkms "$(DESTDIR)$(PREFIX)/lib/audiofuse-control-center/audiofuse8pre-dkms"
	install -d "$(HELPER_DIR)"
	install -m644 $(HELPER_FILES) "$(HELPER_DIR)/"

install: build install-helper
	install -Dm755 $(or $(CARGO_TARGET_DIR),target)/release/audiofuse-control-center $(DESTDIR)$(BINDIR)/audiofuse-control-center
	install -Dm755 $(or $(CARGO_TARGET_DIR),target)/release/audiofuse-cli $(DESTDIR)$(BINDIR)/audiofuse-cli
	install -Dm644 udev/70-arturia-audiofuse.rules $(DESTDIR)$(UDEVDIR)/70-arturia-audiofuse.rules
	install -Dm644 packaging/audiofuse-control-center.desktop $(DESTDIR)$(APPDIR)/audiofuse-control-center.desktop
	install -Dm644 packaging/audiofuse-control-center.svg $(DESTDIR)$(ICONDIR)/audiofuse-control-center.svg

uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/lib/modules-load.d/audiofuse8pre-ctl.conf" "$(DESTDIR)$(PREFIX)/lib/audiofuse-control-center/audiofuse8pre-dkms"
	rm -f $(addprefix $(HELPER_DIR)/,$(notdir $(HELPER_FILES)))
	-rmdir "$(HELPER_DIR)"
	rm -f $(DESTDIR)$(BINDIR)/audiofuse-control-center
	rm -f $(DESTDIR)$(BINDIR)/audiofuse-cli
	rm -f $(DESTDIR)$(UDEVDIR)/70-arturia-audiofuse.rules
	rm -f $(DESTDIR)$(APPDIR)/audiofuse-control-center.desktop
	rm -f $(DESTDIR)$(ICONDIR)/audiofuse-control-center.svg

udev-reload:
	sudo cp udev/70-arturia-audiofuse.rules /etc/udev/rules.d/70-arturia-audiofuse.rules
	sudo udevadm control --reload-rules
	sudo udevadm trigger --subsystem-match=usb --attr-match=idVendor=1c75 --attr-match=idProduct=af0e

appimage:
	bash packaging/appimage/build-appimage.sh

deb:
	bash packaging/debian/build-deb.sh
