# `Makefile` builds, tests, checks, formats and lints the Warped game (the workspace in this
# folder), and says what the machine it runs on can test (REFACTOR_PLAN.md, sections 7 and 8).
#
# Interface:
#     make help: list the targets.
#     make serve-game PORT=<n> MAP=<lunar-surface> SERVER_SETTINGS=<file>: run a game server, without a window, and print how to join it.
#     make join-game SERVER=<auto|address> PORT=<n> MAP=<map> PLAYER_SETTINGS=<file>: open the window and join that game server.
#     (GAME_PASSWORD_FILE, with serve-game and join-game: a file holding the game's password; only players who know it may join.)
#     make build-server-bundle: build the game server alone for x86-64 Linux into $(SERVER_BUNDLE), to upload to a server.
#     make package-mac-app ONLINE_GAMES=<file>: build Warped.app (icon, ad-hoc signature) and zip it with its note into $(APP_ZIP); ONLINE_GAMES is an online_games.txt for its title screen.
#     make bench-views RENDERER=<metal> MAP=<map> WIDTH_PX=<n> HEIGHT_PX=<n> FRAME_COUNT=<n>: time fixed views.
#     make test: check formatting, lint with clippy, and run every crate's tests.
#     make fmt: format with rustfmt and lint with clippy.
#     make check-linux: check that warped-core and warped-server build for x86-64 Linux.
#     make check-environment: print the macOS version, chip, Metal device and toolchain versions.
#     make clean: remove .build/.
#     make reset-artifacts YES_TO_RESET_ARTIFACTS=1: delete .artifacts/.
#
# Example:
#     make test
LINUX_TARGET := x86_64-unknown-linux-musl
RENDERER ?= metal
WIDTH_PX ?= 1280
HEIGHT_PX ?= 800
FRAME_COUNT ?= 10
PORT ?= 47800
MAP ?= lunar-surface
SERVER ?= auto
SERVER_SETTINGS ?=
SERVER_SETTINGS_FLAG := $(if $(SERVER_SETTINGS),--server-settings $(SERVER_SETTINGS))
PLAYER_SETTINGS ?=
PLAYER_SETTINGS_FLAG := $(if $(PLAYER_SETTINGS),--player-settings $(PLAYER_SETTINGS))
GAME_PASSWORD_FILE ?=
GAME_PASSWORD_FLAG := $(if $(GAME_PASSWORD_FILE),--game-password-file $(GAME_PASSWORD_FILE))
RUN_WINDOW := cargo run --release -p warped-client --bin warped --
SERVER_PROGRAM := wormholes_server
SERVER_BUNDLE := .build/server_bundle.tar.gz
APP_FOLDER := .build/Warped
APP := $(APP_FOLDER)/Warped.app
APP_ZIP := .build/Warped.zip
APP_ICONSET := .build/AppIcon.iconset
APP_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
APP_FILES := app
ONLINE_GAMES ?=
FPS_DEMO_ASSETS := ../demos/fps_demo/assets
MAP_MODELS := ../common/maps/models

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (test logs, benchmark results, compared views). \
  Recreating them means rerunning the targets that wrote them. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help serve-game join-game build-server-bundle package-mac-app bench-views test fmt check-linux check-environment clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make serve-game PORT=47800 MAP=lunar-surface     Run a game server, without a window; prints how to join"
	@echo "                                                   (SERVER_SETTINGS=my_server.yaml: a copy of client/server_settings.yaml)"
	@echo "  make join-game SERVER=auto PORT=47800            Open the window and join the game server (SERVER=auto finds one)"
	@echo "                                                   (PLAYER_SETTINGS=mine.yaml: a copy of client/player_settings.yaml)"
	@echo "  (serve-game and join-game take GAME_PASSWORD_FILE=password.txt: only players who know it join)"
	@echo "  make build-server-bundle                         Build the game server alone for x86-64 Linux ($(SERVER_BUNDLE))"
	@echo "  make package-mac-app ONLINE_GAMES=games.txt      Build Warped.app and zip it ($(APP_ZIP)); ONLINE_GAMES is optional"
	@echo "  make bench-views RENDERER=metal WIDTH_PX=1280 HEIGHT_PX=800 FRAME_COUNT=10"
	@echo "                                                   Time fixed views without a window (client/.artifacts/bench_views/)"
	@echo "  make test                                        Check formatting, lint, and run every crate's tests"
	@echo "  make fmt                                         Format with rustfmt and lint with clippy"
	@echo "  make check-linux                                 Check that warped-core and warped-server build for Linux"
	@echo "  make check-environment                           Print macOS, chip, Metal device and toolchain versions"
	@echo "  make clean                                       Remove .build/"
	@echo "  make reset-artifacts YES_TO_RESET_ARTIFACTS=1    Delete .artifacts/ (or INTERACTIVE=1 to be asked)"
	@echo ""
	@echo "Example: make test"

serve-game:
	$(RUN_WINDOW) --serve --port $(PORT) --map $(MAP) $(SERVER_SETTINGS_FLAG) $(GAME_PASSWORD_FLAG)

join-game:
	$(RUN_WINDOW) --join $(SERVER) --port $(PORT) --map $(MAP) $(PLAYER_SETTINGS_FLAG) $(GAME_PASSWORD_FLAG)

build-server-bundle:
	rustup target add $(LINUX_TARGET)
	cargo build --release --target $(LINUX_TARGET) -p warped-client --bin $(SERVER_PROGRAM)
	COPYFILE_DISABLE=1 tar -czf $(SERVER_BUNDLE) -C .build/$(LINUX_TARGET)/release $(SERVER_PROGRAM)
	@echo "Server bundle: $(SERVER_BUNDLE)"

package-mac-app:
	cargo build --release -p warped-client --bin warped
	rm -rf $(APP_FOLDER) $(APP_ICONSET)
	mkdir -p $(APP)/Contents/MacOS $(APP)/Contents/Resources $(APP_ICONSET)
	cp .build/release/warped $(APP)/Contents/MacOS/Warped
	sed 's/@VERSION@/$(APP_VERSION)/' $(APP_FILES)/Info.plist > $(APP)/Contents/Info.plist
	for size in 16 32 128 256 512; do \
		sips -z $$size $$size $(APP_FILES)/app_icon.png --out $(APP_ICONSET)/icon_$${size}x$${size}.png > /dev/null; \
		sips -z $$((size * 2)) $$((size * 2)) $(APP_FILES)/app_icon.png --out $(APP_ICONSET)/icon_$${size}x$${size}@2x.png > /dev/null; \
	done
	iconutil -c icns $(APP_ICONSET) -o $(APP)/Contents/Resources/AppIcon.icns
	$(if $(ONLINE_GAMES),cp $(ONLINE_GAMES) $(APP)/Contents/Resources/online_games.txt)
	mkdir -p $(APP)/Contents/Resources/assets/models $(APP)/Contents/Resources/models
	cp -R $(FPS_DEMO_ASSETS)/textures $(APP)/Contents/Resources/assets/textures
	cp -R $(FPS_DEMO_ASSETS)/models/bolt_action_rifle_7_62 $(APP)/Contents/Resources/assets/models/
	rsync -a --exclude teal_robot $(MAP_MODELS)/ $(APP)/Contents/Resources/models/
	cp $(APP_FILES)/how_to_open.txt "$(APP_FOLDER)/How to open Warped.txt"
	codesign --force --deep --sign - $(APP)
	ditto -c -k --keepParent $(APP_FOLDER) $(APP_ZIP).partial
	mv $(APP_ZIP).partial $(APP_ZIP)
	@echo "App: $(APP)"
	@echo "Zip: $(APP_ZIP)"

bench-views:
	$(RUN_WINDOW) --renderer $(RENDERER) --map $(MAP) --bench-views --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX) --frame-count $(FRAME_COUNT)

test:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --release -- -D warnings
	cargo test --workspace --release

fmt:
	cargo fmt --all
	cargo clippy --workspace --all-targets --release -- -D warnings

check-linux:
	rustup target add $(LINUX_TARGET)
	cargo check -p warped-core -p warped-server --release --target $(LINUX_TARGET)

check-environment:
	@sw_vers
	@echo "chip: $$(sysctl -n machdep.cpu.brand_string)"
	@echo "Metal:"; system_profiler SPDisplaysDataType 2>/dev/null | grep -E 'Chipset Model|Metal' || echo "  no Metal device found"
	@rustc --version; cargo --version
	@cmake --version 2>/dev/null | head -1 || echo "cmake: missing (brew install cmake)"
	@echo "Rust targets: $$(rustup target list --installed | tr '\n' ' ')"

clean:
	rm -rf .build

reset-artifacts:
ifeq ($(YES_TO_RESET_ARTIFACTS),1)
	rm -rf .artifacts
else ifeq ($(INTERACTIVE),1)
	@printf '%s\nType yes to confirm: ' "$(RESET_ARTIFACTS_WARNING)"; read answer; [ "$$answer" = yes ]
	rm -rf .artifacts
else
	@printf '%s\nTo be asked instead, pass INTERACTIVE=1.\n' "$(RESET_ARTIFACTS_WARNING)" >&2; exit 2
endif
