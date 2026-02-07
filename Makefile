PREFIX ?= $(HOME)/.local
BINDIR = $(PREFIX)/bin
APPDIR = $(PREFIX)/share/applications
ICONDIR = $(PREFIX)/share/icons/hicolor/scalable/apps
CONFDIR = $(HOME)/.config/flux

.PHONY: install

install:
	# Add $(DESTDIR) to all directory creations
	@mkdir -p $(DESTDIR)$(BINDIR) $(DESTDIR)$(APPDIR) $(DESTDIR)$(ICONDIR)
	
	# 1. Install Binary (with DESTDIR)
	@install -m 755 target/release/flux $(DESTDIR)$(BINDIR)/flux
	
	# 2. Generate the desktop file
	# Note: We keep @BIN_PATH@ pointing to $(BINDIR) (the final path), 
	# but we write the file to $(DESTDIR)$(APPDIR) (the temporary path).
	@sed "s|@BIN_PATH@|$(BINDIR)/flux|g" flux.desktop.in > flux.desktop.tmp
	
	# 3. Install the generated file (with DESTDIR)
	@install -m 644 flux.desktop.tmp $(DESTDIR)$(APPDIR)/flux.desktop
	@rm flux.desktop.tmp
	
	# 4. Install Icon (with DESTDIR)
	@install -m 644 flux.svg $(DESTDIR)$(ICONDIR)/flux.svg

	# 5. Install default theme as style.css
	@install -m 644 themes/default.css $(DESTDIR)$(CONFDIR)/style.css
	
	# 6. Refresh (Only if not in a fakeroot/DESTDIR environment)
	@if [ -z "$(DESTDIR)" ]; then \
		update-desktop-database $(APPDIR); \
		echo "Successfully installed to $(APPDIR)/flux.desktop"; \
	else \
		echo "Files installed to $(DESTDIR) for packaging."; \
	fi
