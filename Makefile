PREFIX ?= $(HOME)/.local
BIN := target/release/tanim
CARGO ?= $(shell command -v cargo 2>/dev/null || echo $(HOME)/.cargo/bin/cargo)

.PHONY: build install uninstall check videos streamdeck streamdeck-publish html clean

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

# Push streamdeck/ as the only commit of the gh-pages-assets branch, which the
# Pages workflow serves at streamdeck/ so the web version can offer the GIFs.
# An orphan commit, force-pushed, so old GIFs do not pile up in the history.
ASSETS_BRANCH := gh-pages-assets
streamdeck-publish:
	@test -d streamdeck || { echo "no streamdeck/ yet: run make streamdeck first"; exit 1; }
	@index="$$(git rev-parse --absolute-git-dir)/streamdeck.index" && rm -f "$$index" && \
	GIT_INDEX_FILE="$$index" git --work-tree=streamdeck add -A -- '*.gif' && \
	tree=$$(GIT_INDEX_FILE="$$index" git write-tree) && rm -f "$$index" && \
	commit=$$(git commit-tree "$$tree" -m "Stream Deck GIFs from $$(git rev-parse --short HEAD)") && \
	git push --force origin "$$commit:refs/heads/$(ASSETS_BRANCH)"
	@echo "Pushed $(ASSETS_BRANCH); run the Pages workflow (gh workflow run pages.yml) to publish it"

videos: build
	python3 scripts/video.py --seed 7

clean:
	$(CARGO) clean
