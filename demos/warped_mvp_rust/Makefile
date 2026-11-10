# `Makefile` builds, runs, tests, formats and lints Warped's one-window app (`warped`), and builds
# the headless programs it starts when the player hosts a game (the match server and the levels).
#
# Interface:
#     make help: list the targets.
#     make run-warped WIDTH_PX=<n> HEIGHT_PX=<n> PLAYER_SETTINGS=<file>: build the children, then open the app.
#     make build-children: build the match server (../warped_match_rust) and the five levels it starts.
#     make save-screen-pictures WIDTH_PX=<n> HEIGHT_PX=<n>: draw every screen without a window into .artifacts/screen_pictures/.
#     make test: run the tests.
#     make fmt: format with rustfmt and lint with clippy.
#     make clean: remove .build/.
#     make reset-artifacts YES_TO_RESET_ARTIFACTS=1: delete .artifacts/ (children's logs, screen pictures).
#
# Example:
#     make run-warped WIDTH_PX=2560 HEIGHT_PX=1440
WIDTH_PX ?= 1280
HEIGHT_PX ?= 800
PLAYER_SETTINGS ?=
PLAYER_SETTINGS_FLAG := $(if $(PLAYER_SETTINGS),--player-settings $(PLAYER_SETTINGS))
MATCH_FOLDER := ../warped_match_rust

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (the logs of the games this app hosted, and screen pictures). \
  Recreating them means hosting those games and running make save-screen-pictures again. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help run-warped build-children save-screen-pictures test fmt clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make run-warped WIDTH_PX=1280 HEIGHT_PX=800      Build the children, then open the app"
	@echo "                                                   (PLAYER_SETTINGS=mine.yaml: a copy of player_settings.yaml)"
	@echo "  make build-children                              Build the match server and the five levels it starts"
	@echo "  make save-screen-pictures WIDTH_PX=2560 HEIGHT_PX=1440"
	@echo "                                                   Draw every screen without a window (.artifacts/screen_pictures/)"
	@echo "  make test                                        Run the tests"
	@echo "  make fmt                                         Format with rustfmt and lint with clippy"
	@echo "  make clean                                       Remove .build/"
	@echo "  make reset-artifacts YES_TO_RESET_ARTIFACTS=1    Delete .artifacts/ (or INTERACTIVE=1 to be asked)"
	@echo ""
	@echo "Example: make run-warped WIDTH_PX=2560 HEIGHT_PX=1440"

build-children:
	$(MAKE) -C $(MATCH_FOLDER) levels
	cd $(MATCH_FOLDER) && cargo build --release

run-warped: build-children
	cargo run --release -- --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX) $(PLAYER_SETTINGS_FLAG)

save-screen-pictures:
	cargo run --release -- --save-screen-pictures --width-px $(WIDTH_PX) --height-px $(HEIGHT_PX)

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
