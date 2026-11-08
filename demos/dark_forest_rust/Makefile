# `Makefile` downloads the textures for, builds, runs, serves, benchmarks, tests, and formats and lints the
# Dark Forest demo.
#
# Interface:
#     make help: list the targets.
#     make download-assets: download the Poly Haven forest textures into assets/textures/.
#     make run-dark-forest BOT_COUNT=<0..6>: build and play alone against AI robots.
#     make serve-game PORT=<port>: run a game server without a window until killed.
#     make join-game SERVER=<address> PORT=<port> TEAM=<blue|red>: join a game server.
#     make bench-dark-forest BENCH_SECONDS=<s>: walk the vertical route full screen and write frame times to
#         .artifacts/bench_dark_forest/<run_id>/.
#     make bench-views BENCH_SECONDS=<s>: hold each fixed view for BENCH_SECONDS full screen and write frame times
#         to .artifacts/bench_views/<run_id>/.
#     make export-map: write the forest's map to ../../common/maps/dark_forest/map.json.
#     make map-picture: render the forest from above into assets/map_picture.png for the menu's map page.
#     make test: run the tests.
#     make fmt: format with rustfmt and lint with clippy.
#     make clean: remove .build/.
#     make reset-artifacts YES_TO_RESET_ARTIFACTS=1: delete .artifacts/.
#
# Example:
#     make join-game SERVER=127.0.0.1 TEAM=red
SHELL := /bin/bash
BOT_COUNT ?= 3
BENCH_SECONDS ?= 60
PORT ?= 47840
SERVER ?= 127.0.0.1
TEAM ?=
TEAM_FLAG = $(if $(TEAM),--team $(TEAM),)
RUN_ID = $(shell date -u +%Y%m%d-%H%M%S)
BENCH_COMMAND = ./.build/release/dark-forest --play-route --fullscreen --show-frame-time \
  --benchmark-seconds $(BENCH_SECONDS) --bot-count $(BOT_COUNT)
BENCH_VIEWS_COMMAND = ./.build/release/dark-forest --fullscreen --bench-views $(BENCH_SECONDS)

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (frame-time logs, screenshots and videos). \
  Recreating them means rerunning the benchmarks. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help download-assets run-dark-forest serve-game join-game bench-dark-forest bench-views export-map map-picture \
  test fmt clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make download-assets                          Download the forest textures into assets/textures/"
	@echo "  make run-dark-forest BOT_COUNT=<0..6>         Build and play alone (default $(BOT_COUNT) robots)"
	@echo "  make serve-game PORT=<port>                   Run a game server (default port $(PORT))"
	@echo "  make join-game SERVER=<address> TEAM=<team>   Join a game server (default $(SERVER))"
	@echo "  make bench-dark-forest BENCH_SECONDS=<s>      Route benchmark into .artifacts/bench_dark_forest/"
	@echo "  make bench-views BENCH_SECONDS=<s>            Fixed-view benchmark into .artifacts/bench_views/"
	@echo "  make export-map                               Write common/maps/dark_forest/map.json"
	@echo "  make map-picture                              Render the map page's picture into assets/"
	@echo "  make test                                     Run the tests"
	@echo "  make fmt                                      Format with rustfmt and lint with clippy"
	@echo "  make clean                                    Remove .build/"
	@echo "  make reset-artifacts YES_TO_RESET_ARTIFACTS=1 Delete .artifacts/"
	@echo ""
	@echo "Example: make join-game SERVER=127.0.0.1 TEAM=red"

download-assets:
	cargo run --release --bin download-assets

run-dark-forest:
	cargo run --release --bin dark-forest -- --bot-count $(BOT_COUNT)

serve-game:
	cargo run --release --bin dark-forest -- --serve --port $(PORT)

join-game:
	cargo run --release --bin dark-forest -- --join $(SERVER) --port $(PORT) $(TEAM_FLAG)

bench-dark-forest:
	cargo build --release --bin dark-forest
	@set -o pipefail; run_dir=".artifacts/bench_dark_forest/$(RUN_ID)"; mkdir -p "$$run_dir"; \
	  printf '%s\n%s\n' "$(BENCH_COMMAND)" "$$(git describe --always --dirty)" > "$$run_dir/command.txt"; \
	  $(BENCH_COMMAND) 2>&1 | tee "$$run_dir/output.txt" && echo "wrote $$run_dir"

bench-views:
	cargo build --release --bin dark-forest
	@set -o pipefail; run_dir=".artifacts/bench_views/$(RUN_ID)"; mkdir -p "$$run_dir"; \
	  printf '%s\n%s\n' "$(BENCH_VIEWS_COMMAND)" "$$(git describe --always --dirty)" > "$$run_dir/command.txt"; \
	  $(BENCH_VIEWS_COMMAND) 2>&1 | tee "$$run_dir/output.txt" && echo "wrote $$run_dir"

export-map:
	cargo run --release --bin dark-forest -- --export-map ../../common/maps/dark_forest/map.json

map-picture:
	cargo run --release --bin dark-forest -- --width-pt 1600 --height-pt 900 --render-scale 2.0 \
	  --map-picture assets/map_picture.png

test:
	cargo test --release

fmt:
	cargo fmt
	cargo clippy --release --all-targets -- -D warnings

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
