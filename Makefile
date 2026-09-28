PREFIX ?= $(HOME)/.local
BIN := target/release/tanim
CARGO ?= $(shell command -v cargo 2>/dev/null || echo $(HOME)/.cargo/bin/cargo)

.PHONY: build install uninstall check videos streamdeck html clean

build:
	$(CARGO) build --release

install: build
	install -d $(PREFIX)/bin
	install -m 755 $(BIN) $(PREFIX)/bin/tanim
	@echo "Installed $(PREFIX)/bin/tanim"

uninstall:
	rm -f $(PREFIX)/bin/tanim

check: build
	$(BIN) --check

html: build
	$(BIN) --export-html html

# Stream Deck screensaver GIFs; pick models with MODELS=neo,mk2 (default all).
MODELS ?= all
streamdeck: build
	python3 scripts/video.py --streamdeck $(MODELS) --seed 7

videos: build
	python3 scripts/video.py --seed 7

clean:
	$(CARGO) clean
