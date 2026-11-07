# `Makefile` runs the management server with the square game, runs its tests, and formats and
# lints its code.
#
# Interface:
#     make help: list the targets.
#     make run-management-server [ADVERTISED_HOST=<ip>] [PORT=<port>]: serve game servers on the
#         LAN; ADVERTISED_HOST defaults to this Mac's Wi-Fi IP.
#     make test: run the tests.
#     make fmt: format and lint with ruff.
#
# Example:
#     make run-management-server
RUFF ?= uv run ruff
PORT ?= 8000
ADVERTISED_HOST ?= $(shell ipconfig getifaddr en0 2>/dev/null || echo 127.0.0.1)
GAME_SERVER_DIR := $(abspath ../game_server)

.PHONY: help run-management-server test fmt

help:
	@echo "Targets:"
	@echo "  make run-management-server   Serve square-game servers on the LAN"
	@echo "                               (ADVERTISED_HOST=$(ADVERTISED_HOST), PORT=$(PORT))"
	@echo "  make test                    Run the tests"
	@echo "  make fmt                     Format and lint the code with ruff"
	@echo ""
	@echo "Example: make run-management-server"

run-management-server:
	uv sync --quiet --project $(GAME_SERVER_DIR)
	uv run management-server --port $(PORT) --advertised-host $(ADVERTISED_HOST) \
		--game-server-command "$(GAME_SERVER_DIR)/.venv/bin/game-server"

test:
	uv run pytest

fmt:
	$(RUFF) format .
	$(RUFF) check --fix .
