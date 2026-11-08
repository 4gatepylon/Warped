# `Makefile` builds, runs, benchmarks, tests, and formats and lints the Rust hyperbolic wormholes
# demo.
#
# Interface:
#     make help: list the targets.
#     make run-demo RENDERER=<metal|cpu> MAP=<house-demo|lunar-surface> SERVER_SETTINGS=<file>: open the window and play.
#     make serve-game PORT=<n> MAP=<map> SERVER_SETTINGS=<file>: run only a game server, without a window, and print how to join it.
#     make host-game PORT=<n> MAP=<map> SERVER_SETTINGS=<file>: start a game server (its own process, no window) and play in it.
#     (SERVER_SETTINGS: a copy of server_settings.yaml with the numbers that shape play; the server sends them to every player.)
#     (PLAYER_SETTINGS, with run-demo, host-game and join-game: a copy of player_settings.yaml with this player's own keys, mouse speed and view.)
#     make join-game SERVER=<auto|address> MAP=<map>: open the window and join a game on the local network.
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
MAP ?= house-demo
SERVER ?= auto
SERVER_SETTINGS ?=
SERVER_SETTINGS_FLAG := $(if $(SERVER_SETTINGS),--server-settings $(SERVER_SETTINGS))
PLAYER_SETTINGS ?=
PLAYER_SETTINGS_FLAG := $(if $(PLAYER_SETTINGS),--player-settings $(PLAYER_SETTINGS))

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (benchmark results, autopilot frames and timings). \
  Recreating them means rerunning make bench-views and make run-autopilot. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help run-demo serve-game host-game join-game bench-views run-autopilot test fmt clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make run-demo RENDERER=metal MAP=house-demo     Open the window and play (MAP=lunar-surface; RENDERER=cpu: house only)"
	@echo "  make serve-game PORT=47800 MAP=lunar-surface     Run only a game server, without a window; prints how to join"
	@echo "  make host-game PORT=47800                        Start a game server (no window) and play in it"
	@echo "                                                   (both take SERVER_SETTINGS=my_server.yaml, a copy of server_settings.yaml)"
	@echo "  (run-demo, host-game and join-game take PLAYER_SETTINGS=mine.yaml, a copy of player_settings.yaml)"
	@echo "  make join-game SERVER=auto                       Open the window and join a game (SERVER=auto finds one)"
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

run-demo:
	cargo run --release -- --renderer $(RENDERER) --map $(MAP) $(SERVER_SETTINGS_FLAG) $(PLAYER_SETTINGS_FLAG)

serve-game:
	cargo run --release -- --serve --port $(PORT) --map $(MAP) $(SERVER_SETTINGS_FLAG)

host-game:
	cargo run --release -- --host --port $(PORT) --map $(MAP) $(SERVER_SETTINGS_FLAG) $(PLAYER_SETTINGS_FLAG)

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
