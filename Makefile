PREFIX    ?= $(HOME)/.local
BINDIR     = $(PREFIX)/bin
APPDIR     = $(PREFIX)/share/applications
ICONDIR    = $(PREFIX)/share/icons/hicolor/scalable/apps
CONFDIR    = $(PREFIX)/share/flux
SCRIPTDIR  = $(CONFDIR)/scripts
LOCALEDIR  = $(PREFIX)/share/locale

# po/ directory relative to this Makefile
PO_DIR     = po
PO_FILES   = $(wildcard $(PO_DIR)/*.po)
# Derive language tags from filenames: po/pt_BR.po → pt_BR
LANGS      = $(basename $(notdir $(PO_FILES)))

.PHONY: install translations

install: translations
	# 1. Create directories
	@mkdir -p \
		$(DESTDIR)$(BINDIR) \
		$(DESTDIR)$(APPDIR) \
		$(DESTDIR)$(ICONDIR) \
		$(DESTDIR)$(CONFDIR)/themes \
		$(DESTDIR)$(SCRIPTDIR)

	# 2. Binary
	@install -m 755 target/release/flux-fm $(DESTDIR)$(BINDIR)/flux-fm

	# 3. Desktop file
	@sed "s|@BIN_PATH@|$(BINDIR)/flux-fm|g" flux.desktop.in > flux.desktop.tmp
	@install -m 644 flux.desktop.tmp $(DESTDIR)$(APPDIR)/flux.desktop
	@rm -f flux.desktop.tmp

	# 4. Icon
	@install -m 644 flux.svg $(DESTDIR)$(ICONDIR)/flux.svg

	# 5. Themes
	@cp -r themes/. $(DESTDIR)$(CONFDIR)/themes/
	@cp themes/default.css $(DESTDIR)$(CONFDIR)/style.css

	# 6. Scripts
	@install -m 755 scripts/*.py $(DESTDIR)$(SCRIPTDIR)/

	# 7. Refresh desktop database (skip when packaging)
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
