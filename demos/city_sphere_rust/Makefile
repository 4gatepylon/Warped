# `Makefile` builds, runs, benchmarks, tests, and formats and lints the Rust City Sphere demo.
#
# Interface:
#     make help: list the targets.
#     make play-game PORT=<n> TEAM=<blue|red>: open the window and play, hosting a game server on PORT that others (or make run-bot) can join.
#     make serve-game PORT=<n> MAP=<city-sphere|lunar-surface|house-demo> SERVER_SETTINGS=<file>: run a game server, without a window, and print how to join it.
#     make join-game SERVER=<auto|address> PORT=<n> MAP=<map> PLAYER_SETTINGS=<file>: open the window and join that game server.
#     (SERVER_SETTINGS: a copy of server_settings.yaml with the numbers that shape play; the server sends them to every player.)
#     (PLAYER_SETTINGS: a copy of player_settings.yaml with this player's own keys, mouse speed and view.)
#     make run-bot SERVER=<address> PORT=<n> TEAM=<blue|red> BOT_S=<s> BOT_OBJECTIVE=<consoles|target>: a bot without a window joins the game (target: stands still in front of blue's spawn for ROUTE=rifle).
#     make play-route WIDTH_PX=<n> HEIGHT_PX=<n> ROUTE=<legs|rifle>: play the scripted route in the window (legs: lift, reflected shots, both red consoles, blue wins; rifle: overheat, cool, eliminate a BOT_OBJECTIVE=target bot by head and by body hits) and print its numbers.
#     (GAME_PASSWORD_FILE, with serve-game and join-game: a file holding the game's password; only players who know it may join.)
#     make build-server-bundle: build the game server alone for x86-64 Linux into $(SERVER_BUNDLE), to upload to a server.
#     make package-mac-app ONLINE_GAMES=<file>: build Warped.app (icon, ad-hoc signature) and zip it with its note into $(APP_ZIP); ONLINE_GAMES is an online_games.txt for its title screen.
#     make bench-views RENDERER=<metal|cpu> MAP=<map> WIDTH_PX=<n> HEIGHT_PX=<n> FRAME_COUNT=<n>: time fixed views.
#     make download-assets: download the space station's Poly Haven CC0 metal textures into assets/textures/.
#     make map-picture: render the menu's map picture (the sphere seen from its centre, unrolled) into assets/map_picture.png.
#     make photos WIDTH_PX=<n> HEIGHT_PX=<n>: render City Sphere's photogenic views once each into .artifacts/photos/.
#     make run-autopilot RENDERER=<metal|cpu> WIDTH_PX=<n> HEIGHT_PX=<n>: play a scripted route.
#     make test: run the tests.
#     make fmt: format with rustfmt and lint with clippy.
#     make clean: remove .build/.
#     make reset-artifacts YES_TO_RESET_ARTIFACTS=1: delete .artifacts/.
#
# Example:
#     make bench-views RENDERER=cpu WIDTH_PX=2560 HEIGHT_PX=1600
RENDERER ?= metal
WIDTH_PX ?= 1280
HEIGHT_PX ?= 800
FRAME_COUNT ?= 10
AUTOPILOT_WIDTH_PX ?= 640
AUTOPILOT_HEIGHT_PX ?= 400
PORT ?= 47810
ROUTE ?= legs
STATION_TEXTURE_IDS := blue_metal_plate metal_plate_02
MAP ?= city-sphere
TEAM ?=
TEAM_FLAG := $(if $(TEAM),--team $(TEAM))
BOT_S ?= 60
BOT_OBJECTIVE ?=
BOT_OBJECTIVE_FLAG := $(if $(BOT_OBJECTIVE),--bot-objective $(BOT_OBJECTIVE))
SERVER ?= auto
SERVER_SETTINGS ?=
SERVER_TARGET := x86_64-unknown-linux-musl
SERVER_BUNDLE := .build/server_bundle.tar.gz
APP_FOLDER := .build/Warped
APP := $(APP_FOLDER)/Warped.app
APP_ZIP := .build/Warped.zip
APP_ICONSET := .build/AppIcon.iconset
APP_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
ONLINE_GAMES ?=
FPS_DEMO_ASSETS := ../fps_demo/assets
MAP_MODELS := ../../common/maps/models
SERVER_SETTINGS_FLAG := $(if $(SERVER_SETTINGS),--server-settings $(SERVER_SETTINGS))
PLAYER_SETTINGS ?=
PLAYER_SETTINGS_FLAG := $(if $(PLAYER_SETTINGS),--player-settings $(PLAYER_SETTINGS))
GAME_PASSWORD_FILE ?=
GAME_PASSWORD_FLAG := $(if $(GAME_PASSWORD_FILE),--game-password-file $(GAME_PASSWORD_FILE))

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (benchmark results, autopilot frames and timings). \
  Recreating them means rerunning make bench-views and make run-autopilot. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help download-assets map-picture photos play-game run-bot play-route serve-game join-game build-server-bundle package-mac-app bench-views run-autopilot test fmt clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make play-game PORT=47810 TEAM=blue              Open the window and play, hosting a game server others can join"
	@echo "  make run-bot SERVER=127.0.0.1 TEAM=red BOT_S=60  A bot without a window joins (BOT_OBJECTIVE=consoles: it turns consoles off)"
	@echo "  make play-route WIDTH_PX=1280 HEIGHT_PX=800      Play the scripted route in the window and print its frame times"
	@echo "  make serve-game PORT=47810 MAP=city-sphere       Run a game server, without a window; prints how to join"
	@echo "                                                   (SERVER_SETTINGS=my_server.yaml: a copy of server_settings.yaml)"
	@echo "  make join-game SERVER=auto PORT=47810            Open the window and join the game server (SERVER=auto finds one)"
	@echo "                                                   (PLAYER_SETTINGS=mine.yaml: a copy of player_settings.yaml)"
	@echo "  (serve-game and join-game take GAME_PASSWORD_FILE=password.txt: only players who know it join)"
	@echo "  make build-server-bundle                         Build the game server alone for x86-64 Linux ($(SERVER_BUNDLE))"
	@echo "  make package-mac-app ONLINE_GAMES=games.txt      Build Warped.app and zip it ($(APP_ZIP)); ONLINE_GAMES is optional"
	@echo "  make bench-views RENDERER=metal WIDTH_PX=1280 HEIGHT_PX=800 FRAME_COUNT=10"
	@echo "                                                   Time fixed views without a window (.artifacts/bench_views/)"
	@echo "  make run-autopilot RENDERER=metal AUTOPILOT_WIDTH_PX=640 AUTOPILOT_HEIGHT_PX=400"
	@echo "                                                   Play a scripted route without a window (.artifacts/autopilot/)"
	@echo "  make test                                        Run the tests"
	@echo "  make fmt                                         Format with rustfmt and lint with clippy"
	@echo "  make clean                                       Remove .build/"
	@echo "  make reset-artifacts YES_TO_RESET_ARTIFACTS=1    Delete .artifacts/ (or INTERACTIVE=1 to be asked)"
	@echo ""
	@echo "Example: make bench-views RENDERER=cpu WIDTH_PX=2560 HEIGHT_PX=1600"

play-game:
	cargo run --release -- --port $(PORT) --map $(MAP) --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX) $(TEAM_FLAG) $(SERVER_SETTINGS_FLAG) $(PLAYER_SETTINGS_FLAG) $(GAME_PASSWORD_FLAG)

run-bot:
	cargo run --release -- --join $(SERVER) --port $(PORT) --map $(MAP) --bot --exit-after-s $(BOT_S) $(TEAM_FLAG) $(BOT_OBJECTIVE_FLAG) $(GAME_PASSWORD_FLAG)

play-route:
	cargo run --release -- --map $(MAP) --play-route --route $(ROUTE) --port $(PORT) --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX)

serve-game:
	cargo run --release -- --serve --port $(PORT) --map $(MAP) $(SERVER_SETTINGS_FLAG) $(GAME_PASSWORD_FLAG)

join-game:
	cargo run --release -- --join $(SERVER) --port $(PORT) --map $(MAP) $(TEAM_FLAG) $(PLAYER_SETTINGS_FLAG) $(GAME_PASSWORD_FLAG)

build-server-bundle:
	rustup target add $(SERVER_TARGET)
	cargo build --release --target $(SERVER_TARGET) --bin city_sphere_server
	COPYFILE_DISABLE=1 tar -czf $(SERVER_BUNDLE) -C .build/$(SERVER_TARGET)/release city_sphere_server
	@echo "Server bundle: $(SERVER_BUNDLE)"

package-mac-app:
	cargo build --release
	rm -rf .build/Warped .build/AppIcon.iconset
	mkdir -p $(APP)/Contents/MacOS $(APP)/Contents/Resources $(APP_ICONSET)
	cp .build/release/city-sphere $(APP)/Contents/MacOS/Warped
	sed 's/@VERSION@/$(APP_VERSION)/' app/Info.plist > $(APP)/Contents/Info.plist
	for size in 16 32 128 256 512; do \
		sips -z $$size $$size app/app_icon.png --out $(APP_ICONSET)/icon_$${size}x$${size}.png > /dev/null; \
		sips -z $$((size * 2)) $$((size * 2)) app/app_icon.png --out $(APP_ICONSET)/icon_$${size}x$${size}@2x.png > /dev/null; \
	done
	iconutil -c icns $(APP_ICONSET) -o $(APP)/Contents/Resources/AppIcon.icns
	$(if $(ONLINE_GAMES),cp $(ONLINE_GAMES) $(APP)/Contents/Resources/online_games.txt)
	mkdir -p $(APP)/Contents/Resources/assets/models $(APP)/Contents/Resources/models
	cp -R $(FPS_DEMO_ASSETS)/textures $(APP)/Contents/Resources/assets/textures
	cp -R $(FPS_DEMO_ASSETS)/models/bolt_action_rifle_7_62 $(APP)/Contents/Resources/assets/models/
	rsync -a --exclude teal_robot $(MAP_MODELS)/ $(APP)/Contents/Resources/models/
	cp app/how_to_open.txt "$(APP_FOLDER)/How to open Warped.txt"
	codesign --force --deep --sign - $(APP)
	ditto -c -k --keepParent $(APP_FOLDER) $(APP_ZIP).partial
	mv $(APP_ZIP).partial $(APP_ZIP)
	@echo "App: $(APP)"
	@echo "Zip: $(APP_ZIP)"

map-picture:
	cargo run --release -- --map $(MAP) --map-picture

photos:
	cargo run --release -- --map $(MAP) --photos --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX)

bench-views:
	cargo run --release -- --renderer $(RENDERER) --map $(MAP) --bench-views --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX) --frame-count $(FRAME_COUNT)

run-autopilot:
	cargo run --release -- --renderer $(RENDERER) --autopilot --width-px $(AUTOPILOT_WIDTH_PX) --height-px $(AUTOPILOT_HEIGHT_PX)

download-assets:
	for id in $(STATION_TEXTURE_IDS); do mkdir -p assets/textures/$$id && curl -fsSL -o assets/textures/$$id/$${id}_diff_2k.jpg https://dl.polyhaven.org/file/ph-assets/Textures/jpg/2k/$$id/$${id}_diff_2k.jpg; done

test:
	cargo test --release

fmt:
	cargo fmt
	cargo clippy --release -- -D warnings

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
