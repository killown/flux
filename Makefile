PREFIX     ?= $(HOME)/.local
BINDIR     = $(PREFIX)/bin
APPDIR     = $(PREFIX)/share/applications
ICONDIR    = $(PREFIX)/share/icons/hicolor/scalable/apps
METAINFODIR= $(PREFIX)/share/metainfo
CONFDIR    = $(PREFIX)/share/flux
SCRIPTDIR  = $(CONFDIR)/scripts
LOCALEDIR  = $(PREFIX)/share/locale

# Detect the built binary, prefer local profile, then release, then debug.
# Overridable by an environment variable or `make BIN_PATH=...`. When unset,
# falls back to the first existing path. If none exists, this is empty and
# `install:` below aborts instead of silently copying a stale release binary.
BIN_PATH   ?= $(shell if [ -f target/local/flux-fm ]; then echo target/local/flux-fm; \
                      elif [ -f target/release/flux-fm ]; then echo target/release/flux-fm; \
                      elif [ -f target/debug/flux-fm ]; then echo target/debug/flux-fm; \
                      fi)

# User configuration target directory
USER_CONFDIR = $(HOME)/.config/flux

# po/ directory relative to this Makefile
PO_DIR     = po
PO_FILES   = $(wildcard $(PO_DIR)/*.po)
# Derive language tags from filenames: po/pt_BR.po → pt_BR
LANGS      = $(basename $(notdir $(PO_FILES)))

# Desktop entry ID as installed by `make install`
DESKTOP_ID = io.github.killown.flux.desktop

# Theme Repository
THEME_REPO = https://github.com/killown/flux-themes.git


.PHONY: install install-assets translations set-archive-defaults update-themes

install:
	# 0. Guard: fail loudly when no built binary is found, instead of
	#    silently installing a stale target/release/flux-fm while the user
	#    is on a --profile local build.
	@if [ -z "$(BIN_PATH)" ] || [ ! -f "$(BIN_PATH)" ]; then \
		echo "error: no built binary found (looked for target/local, target/release, target/debug)" >&2; \
		exit 1; \
	fi

	# 2. Binary
	@mkdir -p $(DESTDIR)$(BINDIR)
	@install -m 755 $(BIN_PATH) $(DESTDIR)$(BINDIR)/flux-fm

	# Everything else. A sub-make keeps the guard and the binary ahead of any
	# translation or theme work.
	@$(MAKE) --no-print-directory install-assets

# Installs everything except the binary. build.rs runs this target: a build
# script executes before the crate is compiled, so `install` would copy the
# previous build's binary. Set UPDATE_THEMES=1 to fetch flux-themes first.
install-assets: translations $(if $(UPDATE_THEMES),update-themes)
	# 1. Create directories
	@mkdir -p \
		$(DESTDIR)$(BINDIR) \
		$(DESTDIR)$(APPDIR) \
		$(DESTDIR)$(ICONDIR) \
		$(DESTDIR)$(METAINFODIR) \
		$(DESTDIR)$(CONFDIR)/themes \
		$(DESTDIR)$(CONFDIR)/menus \
		$(DESTDIR)$(CONFDIR)/icons \
		$(DESTDIR)$(SCRIPTDIR)

	# 3. Desktop file
	@sed "s|@BIN_PATH@|$(BINDIR)/flux-fm|g" flux.desktop.in > flux.desktop.tmp
	@install -m 644 flux.desktop.tmp $(DESTDIR)$(APPDIR)/io.github.killown.flux.desktop
	@rm -f flux.desktop.tmp

	# 4. Icon & Template
	@install -m 644 flux.svg $(DESTDIR)$(ICONDIR)/io.github.killown.flux.svg
	@if [ -f template.svg ]; then install -m 644 template.svg $(DESTDIR)$(CONFDIR)/icons/template.svg; fi

	# 4.1 Nerd Font Dataset
	@if [ -f assets/nerd_fonts.json ]; then install -m 644 assets/nerd_fonts.json $(DESTDIR)$(CONFDIR)/nerd_fonts.json; fi

	# 5. AppStream Metainfo
	@install -m 644 packaging/flatpak/io.github.killown.flux.metainfo.xml $(DESTDIR)$(METAINFODIR)/io.github.killown.flux.metainfo.xml

	# 6. Themes & Shared Menus
	@cp -r themes/. $(DESTDIR)$(CONFDIR)/themes/
	@if [ -d menus ]; then cp -r menus/. $(DESTDIR)$(CONFDIR)/menus/; fi

	# 7. Scripts
	@install -m 755 scripts/*.py $(DESTDIR)$(SCRIPTDIR)/

	# 8. Copy default menus to ~/.config/flux/menus/ if not already present
	@if [ -z "$(DESTDIR)" ] && [ -d menus ]; then \
		mkdir -p $(USER_CONFDIR)/menus; \
		for file in menus/*.rs; do \
			if [ -f "$$file" ]; then \
				target="$(USER_CONFDIR)/menus/$$(basename "$$file")"; \
				if [ ! -f "$$target" ]; then \
					cp "$$file" "$$target"; \
					echo "Installed default menu template: $$target"; \
				fi; \
			fi; \
		done; \
	fi

	# 9. Refresh desktop database (skip when packaging)
	@if [ -z "$(DESTDIR)" ]; then \
		if command -v update-desktop-database >/dev/null 2>&1; then \
			update-desktop-database $(PREFIX)/share/applications || \
				echo "Warning: update-desktop-database failed" >&2; \
		else \
			echo "Warning: update-desktop-database not found, skipping" >&2; \
		fi; \
		echo "Successfully installed to $(PREFIX)"; \
	else \
		echo "Files staged to $(DESTDIR) for packaging."; \
	fi

# Clone or pull community themes repository cleanly without nesting
update-themes:
	@mkdir -p $(DESTDIR)$(CONFDIR)/themes
	@TMP_THEME_DIR=$$(mktemp -d); \
	if git clone --quiet --depth 1 $(THEME_REPO) "$$TMP_THEME_DIR"; then \
		if [ -d "$$TMP_THEME_DIR/themes" ]; then \
			cp -r "$$TMP_THEME_DIR/themes/." "$(DESTDIR)$(CONFDIR)/themes/"; \
		else \
			cp -r "$$TMP_THEME_DIR/." "$(DESTDIR)$(CONFDIR)/themes/"; \
		fi; \
		echo "Successfully synced flux-themes."; \
	else \
		echo "Warning: Failed to fetch flux-themes repo (offline?)"; \
	fi; \
	rm -rf "$$TMP_THEME_DIR"

# Compile every po/LANG.po → $(LOCALEDIR)/LANG/LC_MESSAGES/flux.mo
translations:
	@$(foreach lang,$(LANGS), \
		mkdir -p $(DESTDIR)$(LOCALEDIR)/$(lang)/LC_MESSAGES && \
		msgfmt -o $(DESTDIR)$(LOCALEDIR)/$(lang)/LC_MESSAGES/flux.mo \
		       $(PO_DIR)/$(lang).po && \
		echo "Compiled translation: $(lang)" || exit 1; \
	)

# To set flux as the default handler for compressed files:
#   make set-archive-defaults
set-archive-defaults:
	@for m in application/zip application/x-7z-compressed application/x-tar \
	          application/gzip application/x-bzip2 application/x-xz application/zstd \
	          application/x-rar application/vnd.rar application/x-iso9660-image \
	          application/x-compressed-tar application/x-bzip-compressed-tar \
	          application/x-xz-compressed-tar application/x-zstd-compressed-tar; do \
		xdg-mime default $(DESKTOP_ID) $$m || exit 1; \
	done
	update-desktop-database ~/.local/share/applications
