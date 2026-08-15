PREFIX     ?= $(HOME)/.local
BINDIR     = $(PREFIX)/bin
APPDIR     = $(PREFIX)/share/applications
ICONDIR    = $(PREFIX)/share/icons/hicolor/scalable/apps
CONFDIR    = $(PREFIX)/share/flux
SCRIPTDIR  = $(CONFDIR)/scripts
LOCALEDIR  = $(PREFIX)/share/locale

# User configuration target directory
USER_CONFDIR = $(HOME)/.config/flux

# po/ directory relative to this Makefile
PO_DIR     = po
PO_FILES   = $(wildcard $(PO_DIR)/*.po)
# Derive language tags from filenames: po/pt_BR.po → pt_BR
LANGS      = $(basename $(notdir $(PO_FILES)))


.PHONY: install translations set-archive-defaults

install: translations
	# 1. Create directories
	@mkdir -p \
		$(DESTDIR)$(BINDIR) \
		$(DESTDIR)$(APPDIR) \
		$(DESTDIR)$(ICONDIR) \
		$(DESTDIR)$(CONFDIR)/themes \
		$(DESTDIR)$(CONFDIR)/menus \
		$(DESTDIR)$(SCRIPTDIR)

	# 2. Binary
	@install -m 755 target/release/flux-fm $(DESTDIR)$(BINDIR)/flux-fm

	# 3. Desktop file
	@sed "s|@BIN_PATH@|$(BINDIR)/flux-fm|g" flux.desktop.in > flux.desktop.tmp
	@install -m 644 flux.desktop.tmp $(DESTDIR)$(APPDIR)/flux.desktop
	@rm -f flux.desktop.tmp

	# 4. Icon
	@install -m 644 flux.svg $(DESTDIR)$(ICONDIR)/flux.svg

	# 5. Themes & Shared Menus
	@cp -r themes/. $(DESTDIR)$(CONFDIR)/themes/
	@cp themes/default.css $(DESTDIR)$(CONFDIR)/style.css
	@if [ -d menus ]; then cp -r menus/. $(DESTDIR)$(CONFDIR)/menus/; fi

	# 6. Scripts
	@install -m 755 scripts/*.py $(DESTDIR)$(SCRIPTDIR)/

	# 7. Copy default menus to ~/.config/flux/menus/ if not already present
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

	# 8. Refresh desktop database (skip when packaging)
	@if [ -z "$(DESTDIR)" ]; then \
		update-desktop-database $(PREFIX)/share/applications; \
		echo "Successfully installed to $(PREFIX)"; \
	else \
		echo "Files staged to $(DESTDIR) for packaging."; \
	fi

# Compile every po/LANG.po → $(LOCALEDIR)/LANG/LC_MESSAGES/flux.mo
translations:
	@$(foreach lang,$(LANGS), \
		mkdir -p $(DESTDIR)$(LOCALEDIR)/$(lang)/LC_MESSAGES; \
		msgfmt -o $(DESTDIR)$(LOCALEDIR)/$(lang)/LC_MESSAGES/flux.mo \
		       $(PO_DIR)/$(lang).po; \
		echo "Compiled translation: $(lang)"; \
	)

# To set flux as the default handler for compressed files:
#   make set-archive-defaults
set-archive-defaults:
	xdg-mime default flux.desktop application/zip
	xdg-mime default flux.desktop application/x-7z-compressed
	xdg-mime default flux.desktop application/x-tar
	xdg-mime default flux.desktop application/gzip
	xdg-mime default flux.desktop application/x-bzip2
	xdg-mime default flux.desktop application/x-xz
	xdg-mime default flux.desktop application/zstd
	xdg-mime default flux.desktop application/x-rar
	xdg-mime default flux.desktop application/vnd.rar
	xdg-mime default flux.desktop application/x-iso9660-image
	xdg-mime default flux.desktop application/x-compressed-tar
	xdg-mime default flux.desktop application/x-bzip-compressed-tar
	xdg-mime default flux.desktop application/x-xz-compressed-tar
	xdg-mime default flux.desktop application/x-zstd-compressed-tar
	update-desktop-database ~/.local/share/applications
