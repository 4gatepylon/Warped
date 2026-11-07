# `Makefile` downloads the assets for, builds, runs, benchmarks, tests, and formats and lints the
# FPS demo.
#
# Interface:
#     make help: list the targets.
#     make download-assets: download the CC0 assets (and the Sketchfab models if
#         SKETCHFAB_API_TOKEN is set) into .assets/.
#     make run-fps-demo RENDER_SCALE=<0.25..1> BOT_COUNT=<0..6>: build and play.
#     make bench-fps-demo BENCH_SECONDS=<s>: let the autopilot play and write frame times to
#         .artifacts/bench_fps_demo/<run_id>/.
#     make test: run the tests.
#     make fmt: format with rustfmt and lint with clippy.
#     make clean: remove .build/.
#     make reset-artifacts YES_TO_RESET_ARTIFACTS=1: delete .artifacts/.
#
# Example:
#     make download-assets && make run-fps-demo RENDER_SCALE=0.8
RENDER_SCALE ?= 1.0
BOT_COUNT ?= 4
BENCH_SECONDS ?= 30
BENCH_RUN_DIR = .artifacts/bench_fps_demo/$(shell date -u +%Y%m%d-%H%M%S)
BENCH_COMMAND = ./.build/release/warped-fps-demo --autopilot --show-frame-time \
  --benchmark-seconds $(BENCH_SECONDS) --render-scale $(RENDER_SCALE) --bot-count $(BOT_COUNT)

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (frame-time logs and screenshots). \
  Recreating them means rerunning make bench-fps-demo. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help download-assets run-fps-demo bench-fps-demo test fmt clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make download-assets                         Download the assets into .assets/ (once)"
	@echo "  make run-fps-demo RENDER_SCALE=<0.25..1> BOT_COUNT=<0..6>"
	@echo "                                               Build and play (defaults $(RENDER_SCALE), $(BOT_COUNT))"
	@echo "  make bench-fps-demo BENCH_SECONDS=<s>        Autopilot benchmark into .artifacts/bench_fps_demo/"
	@echo "  make test                                    Run the tests"
	@echo "  make fmt                                     Format with rustfmt and lint with clippy"
	@echo "  make clean                                   Remove .build/"
	@echo "  make reset-artifacts YES_TO_RESET_ARTIFACTS=1  Delete .artifacts/"
	@echo ""
	@echo "Example: make download-assets && make run-fps-demo RENDER_SCALE=0.8"

download-assets:
	cargo run --release --bin download-assets

run-fps-demo:
	cargo run --release --bin warped-fps-demo -- --render-scale $(RENDER_SCALE) --bot-count $(BOT_COUNT)

bench-fps-demo:
	cargo build --release --bin warped-fps-demo
	@run_dir="$(BENCH_RUN_DIR)"; mkdir -p "$$run_dir"; \
	  printf '%s\n%s\n' "$(BENCH_COMMAND)" "$$(git describe --always --dirty)" > "$$run_dir/command.txt"; \
	  $(BENCH_COMMAND) 2>&1 | tee "$$run_dir/output.txt"; \
	  echo "wrote $$run_dir"

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
