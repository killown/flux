PREFIX ?= $(HOME)/.local
BINDIR = $(PREFIX)/bin

install:
	install -Dm755 scripts/properties.py $(BINDIR)/file-props
	@echo "Installed to $(BINDIR)/file-props"

uninstall:
	rm -f $(BINDIR)/file-props
