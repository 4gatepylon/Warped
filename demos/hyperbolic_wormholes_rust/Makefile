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
#     make bench-views RENDERER=<metal|cpu> MAP=<map> WIDTH_PX=<n> HEIGHT_PX=<n> FRAME_COUNT=<n>: time fixed views.
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
PORT ?= 47800
MAP ?= lunar-surface
SERVER ?= auto
SERVER_SETTINGS ?=
SERVER_SETTINGS_FLAG := $(if $(SERVER_SETTINGS),--server-settings $(SERVER_SETTINGS))
PLAYER_SETTINGS ?=
PLAYER_SETTINGS_FLAG := $(if $(PLAYER_SETTINGS),--player-settings $(PLAYER_SETTINGS))

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (benchmark results, autopilot frames and timings). \
  Recreating them means rerunning make bench-views and make run-autopilot. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help serve-game join-game bench-views run-autopilot test fmt clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make serve-game PORT=47800 MAP=lunar-surface     Run a game server, without a window; prints how to join"
	@echo "                                                   (SERVER_SETTINGS=my_server.yaml: a copy of server_settings.yaml)"
	@echo "  make join-game SERVER=auto PORT=47800            Open the window and join the game server (SERVER=auto finds one)"
	@echo "                                                   (PLAYER_SETTINGS=mine.yaml: a copy of player_settings.yaml)"
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
	cargo run --release -- --serve --port $(PORT) --map $(MAP) $(SERVER_SETTINGS_FLAG)

join-game:
	cargo run --release -- --join $(SERVER) --port $(PORT) --map $(MAP) $(PLAYER_SETTINGS_FLAG)

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
