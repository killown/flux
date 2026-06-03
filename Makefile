PREFIX ?= $(HOME)/.local
BINDIR = $(PREFIX)/bin
APPDIR = $(PREFIX)/share/applications
ICONDIR = $(PREFIX)/share/icons/hicolor/scalable/apps
CONFDIR = $(PREFIX)/share/flux
SCRIPTDIR = $(CONFDIR)/scripts
LOCALEDIR = $(PREFIX)/share/locale

.PHONY: install

install:
	# Create all necessary directories
	@mkdir -p $(DESTDIR)$(BINDIR) $(DESTDIR)$(APPDIR) $(DESTDIR)$(ICONDIR) $(DESTDIR)$(CONFDIR)/themes $(DESTDIR)$(SCRIPTDIR) $(DESTDIR)$(LOCALEDIR)
	
	# 1. Install Binary
	@install -m 755 target/release/flux-fm $(DESTDIR)$(BINDIR)/flux-fm
	
	# 2. Generate and Install Desktop File
	@sed "s|@BIN_PATH@|$(BINDIR)/flux-fm|g" flux.desktop.in > flux.desktop.tmp
	@install -m 644 flux.desktop.tmp $(DESTDIR)$(APPDIR)/flux.desktop
	@rm flux.desktop.tmp
	
	# 3. Install Icon
	@install -m 644 flux.svg $(DESTDIR)$(ICONDIR)/flux.svg

	# 4. Install the theme library
	@cp -r themes/. $(DESTDIR)$(CONFDIR)/themes/
	
	# 5. Set default style.css
	@cp themes/default.css $(DESTDIR)$(CONFDIR)/style.css

	# 6. Install Scripts
	@install -m 755 scripts/*.py $(DESTDIR)$(SCRIPTDIR)/
	
	# 7. Compile and Install Translations (.po -> .mo)
	@for po in share/locale/*/LC_MESSAGES/*.po; do \
		if [ -f "$$po" ]; then \
			lang=$$(echo $$po | cut -d'/' -f3); \
			mkdir -p $(DESTDIR)$(LOCALEDIR)/$$lang/LC_MESSAGES; \
			msgfmt -o $(DESTDIR)$(LOCALEDIR)/$$lang/LC_MESSAGES/flux.mo $$po; \
			echo "Installed translation for $$lang"; \
		fi \
	done
	
	# 8. Refresh
	@if [ -z "$(DESTDIR)" ]; then \
		update-desktop-database $(PREFIX)/share/applications; \
		echo "Successfully installed to $(PREFIX)"; \
	else \
		echo "Files installed to $(DESTDIR) for packaging."; \
	fi
