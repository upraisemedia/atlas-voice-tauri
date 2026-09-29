# Rust is installed via rustup in ~/.cargo; make sure it is on PATH for every recipe.
export PATH := $(HOME)/.cargo/bin:$(PATH)

TAURI_DIR := src-tauri
BUNDLE_ID := nl.atlasvoice.desktop

.PHONY: install dev app check fmt test lint reset-permissions logs clean

install:            ## Install JS dependencies
	npm install

dev:                ## Hot-reload dev build (unsigned: permissions attach to the terminal)
	npm run tauri dev

app:                ## Signed debug .app, launched (use this to test permissions)
	./scripts/dev-app.sh

check: lint test    ## Everything CI would run

lint:               ## Formatting, clippy, eslint, typecheck
	cd $(TAURI_DIR) && cargo fmt --check
	cd $(TAURI_DIR) && cargo clippy --all-targets -- -D warnings
	npm run format:check
	npm run lint
	npm run typecheck

test:               ## Rust + frontend unit tests
	cd $(TAURI_DIR) && cargo test
	npm test

fmt:                ## Auto-format Rust and TS
	cd $(TAURI_DIR) && cargo fmt
	npm run format

reset-permissions:  ## Forget granted macOS permissions for this app
	tccutil reset Microphone $(BUNDLE_ID) || true
	tccutil reset Accessibility $(BUNDLE_ID) || true
	tccutil reset ListenEvent $(BUNDLE_ID) || true

logs:               ## Follow the app log
	tail -F "$(HOME)/Library/Logs/$(BUNDLE_ID)/"*.log

clean:
	cd $(TAURI_DIR) && cargo clean
	rm -rf dist
