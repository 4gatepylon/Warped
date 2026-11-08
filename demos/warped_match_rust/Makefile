# `Makefile` builds, runs, benchmarks, tests, and formats and lints the Rust hyperbolic wormholes
# demo.
#
# Interface:
#     make help: list the targets.
#     make serve-game PORT=<n> MAP=<lunar-surface|house-demo> SERVER_SETTINGS=<file>: run a game server, without a window, and print how to join it.
#     make join-game SERVER=<auto|address> PORT=<n> MAP=<map> PLAYER_SETTINGS=<file>: open the window and join that game server.
#     (SERVER_SETTINGS: a copy of server_settings.yaml with the numbers that shape play; the server sends them to every player.)
#     (PLAYER_SETTINGS: a copy of player_settings.yaml with this player's own keys, mouse speed and view.)
#     (Playing is only on a game server: there is no local demo and no host-and-play target.)
#     (GAME_PASSWORD_FILE, with serve-game and join-game: a file holding the game's password; only players who know it may join.)
#     make build-server-bundle: build the online match's server for x86-64 Linux into $(SERVER_BUNDLE): the match server, every level's server and app/match_levels.yaml, with services.txt saying what the cloud VM runs (W-127).
#     make package-mac-app SERVER_ADDRESS=<ip>: build Warped.app with every level of app/match_levels.yaml in it (each level's own make package-mac-app) and zip it with its note into $(APP_ZIP); SERVER_ADDRESS puts the online match at <ip> on its title screen.
#     make check-server SERVER_ADDRESS=<ip> GAME_PASSWORD_FILE=<file>: join the match server at <ip> with a bot for $(CHECK_S) s, to check that it answers.
#     make bench-views RENDERER=<metal|cpu> MAP=<map> WIDTH_PX=<n> HEIGHT_PX=<n> FRAME_COUNT=<n>: time fixed views.
#     make progression-pictures WIDTH_PX=<n> HEIGHT_PX=<n>: render the progression page for a few match states into .artifacts/progression_pictures/.
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
PORT ?= 47852
MATCH_LEVELS ?= match_levels.yaml
TEAM ?=
TEAM_FLAG := $(if $(TEAM),--team $(TEAM))
SERVER_MATCH ?= 127.0.0.1
MAP ?= lunar-surface
SERVER ?= auto
SERVER_SETTINGS ?=
SERVER_TARGET := x86_64-unknown-linux-musl
SERVER_BUNDLE := .build/server_bundle.tar.gz
APP_FOLDER := .build/Warped
APP := $(APP_FOLDER)/Warped.app
APP_ZIP := .build/Warped.zip
APP_ICONSET := .build/AppIcon.iconset
APP_VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
SERVER_ADDRESS ?=
MATCH_PORT := 47850
CHECK_S := 5
RELEASE_LEVELS := app/match_levels.yaml
SHIPPED_LEVEL_FOLDERS := dust_rust snow_planet_rust city_sphere_rust dark_forest_rust
SERVER_BUNDLE_FOLDER := .build/server_bundle
VM_PROGRAM_FOLDER := /opt/warped
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

.PHONY: help progression-pictures levels serve-match join-match bot-match serve-game join-game build-server-bundle package-mac-app check-server bench-views run-autopilot test fmt clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make levels                                      Link (git worktree) and build the plugged-in levels' branches in .levels/"
	@echo "  make serve-match                                 Run a match server (UDP 47850) over the levels of match_levels.yaml"
	@echo "  make join-match SERVER=127.0.0.1 TEAM=blue       Open the match window (progression page); each round opens its level"
	@echo "  make bot-match TEAM=red                          Join the match with a bot that turns off the other team's consoles"
	@echo "  make e2e-match                                   A whole match of two bots, consoles off in 1 s (e2e/)"
	@echo "  make levels LEVEL_SOURCE=checkout                Build the levels from this checkout's demos/<level>_rust/ instead of their branches"
	@echo "  make serve-game PORT=47852 MAP=lunar-surface     Run a game server, without a window; prints how to join"
	@echo "                                                   (SERVER_SETTINGS=my_server.yaml: a copy of server_settings.yaml)"
	@echo "  make join-game SERVER=auto PORT=47852            Open the window and join the game server (SERVER=auto finds one)"
	@echo "                                                   (PLAYER_SETTINGS=mine.yaml: a copy of player_settings.yaml)"
	@echo "  (serve-game and join-game take GAME_PASSWORD_FILE=password.txt: only players who know it join)"
	@echo "  make build-server-bundle                         Build the online match's servers for x86-64 Linux ($(SERVER_BUNDLE))"
	@echo "  make package-mac-app SERVER_ADDRESS=34.1.2.3     Build Warped.app with every level and zip it ($(APP_ZIP))"
	@echo "  make check-server SERVER_ADDRESS=34.1.2.3        Join that match server with a bot for $(CHECK_S) s"
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

serve-game:
	cargo run --release -- --serve --port $(PORT) --map $(MAP) $(SERVER_SETTINGS_FLAG) $(GAME_PASSWORD_FLAG)

join-game:
	cargo run --release -- --join $(SERVER) --port $(PORT) --map $(MAP) $(PLAYER_SETTINGS_FLAG) $(GAME_PASSWORD_FLAG)

build-server-bundle:
	rustup target add $(SERVER_TARGET)
	cargo build --release --target $(SERVER_TARGET) --bin wormholes_server --bin match_server
	rm -rf .build/server_bundle
	mkdir -p $(SERVER_BUNDLE_FOLDER)/levels
	cp .build/$(SERVER_TARGET)/release/wormholes_server .build/$(SERVER_TARGET)/release/match_server $(SERVER_BUNDLE_FOLDER)/
	for level in $(SHIPPED_LEVEL_FOLDERS); do \
		$(MAKE) -C ../$$level build-server-bundle SERVER_TARGET=$(SERVER_TARGET) && \
		mkdir -p $(SERVER_BUNDLE_FOLDER)/levels/$$level && \
		tar -xzf ../$$level/.build/server_bundle.tar.gz -C $(SERVER_BUNDLE_FOLDER)/levels/$$level || exit 1; \
	done
	cp $(RELEASE_LEVELS) $(SERVER_BUNDLE_FOLDER)/match_levels.yaml
	echo "$(MATCH_PORT) match_server --level-port $(PORT) --match-levels $(VM_PROGRAM_FOLDER)/match_levels.yaml" \
		> $(SERVER_BUNDLE_FOLDER)/services.txt
	COPYFILE_DISABLE=1 tar -czf $(SERVER_BUNDLE) -C $(SERVER_BUNDLE_FOLDER) .
	@echo "Server bundle: $(SERVER_BUNDLE)"

package-mac-app:
	cargo build --release
	rm -rf .build/Warped .build/AppIcon.iconset
	mkdir -p $(APP)/Contents/MacOS $(APP)/Contents/Resources $(APP_ICONSET)
	cp .build/release/hyperbolic-wormholes $(APP)/Contents/MacOS/Warped
	sed 's/@VERSION@/$(APP_VERSION)/' app/Info.plist > $(APP)/Contents/Info.plist
	for size in 16 32 128 256 512; do \
		sips -z $$size $$size app/app_icon.png --out $(APP_ICONSET)/icon_$${size}x$${size}.png > /dev/null; \
		sips -z $$((size * 2)) $$((size * 2)) app/app_icon.png --out $(APP_ICONSET)/icon_$${size}x$${size}@2x.png > /dev/null; \
	done
	iconutil -c icns $(APP_ICONSET) -o $(APP)/Contents/Resources/AppIcon.icns
	mkdir -p $(APP)/Contents/Resources/assets/models $(APP)/Contents/Resources/models
	cp -R $(FPS_DEMO_ASSETS)/textures $(APP)/Contents/Resources/assets/textures
	cp -R $(FPS_DEMO_ASSETS)/models/bolt_action_rifle_7_62 $(APP)/Contents/Resources/assets/models/
	rsync -a --exclude teal_robot $(MAP_MODELS)/ $(APP)/Contents/Resources/models/
	for level in $(SHIPPED_LEVEL_FOLDERS); do \
		(! grep -q '^download-assets:' ../$$level/Makefile || $(MAKE) -C ../$$level download-assets) && \
		$(MAKE) -C ../$$level package-mac-app && \
		cp ../$$level/.build/Warped/Warped.app/Contents/MacOS/Warped $(APP)/Contents/MacOS/$$level && \
		rsync -a --exclude AppIcon.icns --exclude online_games.txt ../$$level/.build/Warped/Warped.app/Contents/Resources/ \
			$(APP)/Contents/Resources/ || exit 1; \
	done
	cp $(RELEASE_LEVELS) $(APP)/Contents/Resources/match_levels.yaml
	$(if $(SERVER_ADDRESS),echo "$(SERVER_ADDRESS):$(MATCH_PORT) Warped match" > $(APP)/Contents/Resources/online_games.txt)
	cp app/how_to_open.txt "$(APP_FOLDER)/How to open Warped.txt"
	codesign --force --deep --sign - $(APP)
	ditto -c -k --keepParent $(APP_FOLDER) $(APP_ZIP).partial
	mv $(APP_ZIP).partial $(APP_ZIP)
	@echo "App: $(APP)"
	@echo "Zip: $(APP_ZIP)"

progression-pictures:
	cargo run --release -- --progression-pictures --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX)

bench-views:
	cargo run --release -- --renderer $(RENDERER) --map $(MAP) --bench-views --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX) --frame-count $(FRAME_COUNT)

run-autopilot:
	cargo run --release -- --renderer $(RENDERER) --autopilot --width-px $(AUTOPILOT_WIDTH_PX) --height-px $(AUTOPILOT_HEIGHT_PX)

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

# The plugged-in levels (match_levels.yaml reads .levels/<name>/demos/<name>_rust/). LEVEL_SOURCE=branches
# makes .levels/<name> a git worktree of the level's branch tip; LEVEL_SOURCE=checkout makes it a symlink to
# this checkout, so the levels come from demos/<name>_rust/ here (e.g. once merged into staging/user-4gatepylon).
LEVEL_BRANCHES := snow_planet:devin/snow-planet-demo dust:devin/dust-demo city_sphere:devin/city-sphere-demo dark_forest:devin/dark-forest-demo
LEVEL_SOURCE ?= checkout

levels:
ifeq ($(LEVEL_SOURCE),checkout)
	mkdir -p .levels
	$(foreach level,$(LEVEL_BRANCHES),\
	  $(eval name := $(word 1,$(subst :, ,$(level))))\
	  (test -L .levels/$(name) || ! test -e .levels/$(name) || (echo ".levels/$(name) is a branch worktree; remove it with git worktree remove .levels/$(name)" >&2; false)) && \
	  ln -sfn ../../.. .levels/$(name) && \
	  (cd .levels/$(name)/demos/$(name)_rust && (! grep -q '^download-assets:' Makefile || make download-assets) && cargo build --release) &&) true
else ifeq ($(LEVEL_SOURCE),branches)
	git fetch origin $(foreach level,$(LEVEL_BRANCHES),$(word 2,$(subst :, ,$(level))))
	$(foreach level,$(LEVEL_BRANCHES),\
	  $(eval name := $(word 1,$(subst :, ,$(level))))$(eval branch := $(word 2,$(subst :, ,$(level))))\
	  (! test -L .levels/$(name) || rm .levels/$(name)) && \
	  (test -d .levels/$(name) || git worktree add --detach .levels/$(name) origin/$(branch)) && \
	  git -C .levels/$(name) checkout --detach origin/$(branch) && \
	  (cd .levels/$(name)/demos/$(name)_rust && (! grep -q '^download-assets:' Makefile || make download-assets) && cargo build --release) &&) true
else
	@echo "LEVEL_SOURCE must be branches or checkout, got $(LEVEL_SOURCE)" >&2; exit 2
endif

# The match (README, "Match"): a match server, a window or bot per player.
check-server:
	cargo build --release
	.build/release/hyperbolic-wormholes --match $(SERVER_ADDRESS):$(MATCH_PORT) --match-levels $(RELEASE_LEVELS) \
		--bot --team blue --exit-after-s $(CHECK_S) $(GAME_PASSWORD_FLAG)

serve-match: levels
	cargo run --release -- --serve-match --match-levels $(MATCH_LEVELS)

join-match:
	cargo run --release -- --match $(SERVER_MATCH) $(TEAM_FLAG)

bot-match:
	cargo run --release -- --match $(SERVER_MATCH) --bot --team $(or $(TEAM),red)

e2e-match: levels
	cargo build --release
	./.build/release/hyperbolic-wormholes --match 127.0.0.1 --bot --team blue > .build/e2e_blue.log 2>&1 &
	./.build/release/hyperbolic-wormholes --match 127.0.0.1 --bot --team red > .build/e2e_red.log 2>&1 &
	./.build/release/hyperbolic-wormholes --serve-match --match-levels e2e/match_levels_fast.yaml --intermission-s 2 --round-over-s 2 --label e2e
