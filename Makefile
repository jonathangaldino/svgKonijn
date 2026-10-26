export PATH := $(HOME)/.cargo/bin:$(PATH)
# macOS make 3.81 execs simple commands without the exported PATH, so resolve cargo up front.
CARGO ?= $(shell command -v cargo || echo $(HOME)/.cargo/bin/cargo)

BUNDLE := src-tauri/target
DIST := dist
# Empty builds Linux for this machine's arch; `make build-linux LINUX_PLATFORM=linux/amd64` emulates x86_64 (slow).
LINUX_PLATFORM ?=
LLVM_PATH := /opt/homebrew/opt/llvm/bin:/opt/homebrew/opt/lld/bin

.PHONY: dev test build-macos build-windows build-linux build-all clean

node_modules: package-lock.json
	npm ci
	@touch node_modules

dev: node_modules
	npx tauri dev

test:
	$(CARGO) test --lib --manifest-path src-tauri/Cargo.toml

build-macos: node_modules
	npx tauri build --bundles app,dmg
	@mkdir -p $(DIST) && cp $(BUNDLE)/release/bundle/dmg/*.dmg $(DIST)/

# Cross-compiles from macOS; needs: brew install nsis llvm lld && cargo install cargo-xwin && rustup target add x86_64-pc-windows-msvc
build-windows: node_modules
	PATH="$(LLVM_PATH):$$PATH" npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc
	@mkdir -p $(DIST) && cp $(BUNDLE)/x86_64-pc-windows-msvc/release/bundle/nsis/*.exe $(DIST)/

# Builds in Docker; the named volume caches downloaded crates between runs.
build-linux:
	@mkdir -p $(DIST)
	docker run --rm $(if $(LINUX_PLATFORM),--platform $(LINUX_PLATFORM)) \
		-v "$(CURDIR)":/src:ro -v "$(CURDIR)/$(DIST)":/out \
		-v svgkonijn-cargo:/usr/local/cargo/registry \
		rust:1-bookworm bash /src/scripts/build-linux.sh

build-all: build-macos build-windows build-linux

clean:
	rm -rf $(DIST)
	$(CARGO) clean --manifest-path src-tauri/Cargo.toml
