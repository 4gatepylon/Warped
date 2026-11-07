# `Makefile` checks the maps against the map format, runs the validator's tests, and formats and
# lints its code.
#
# Interface:
#     make help: list the targets.
#     make validate-maps: check every map folder here against README.md and map.schema.json.
#     make test: run the validator's tests.
#     make fmt: format and lint with ruff.
#
# Example:
#     make validate-maps
RUFF ?= uv run ruff
MAP_FOLDERS := $(patsubst %/map.json,%,$(wildcard */map.json))

.PHONY: help validate-maps test fmt

help:
	@echo "Targets:"
	@echo "  make help            List the targets"
	@echo "  make validate-maps   Check every map folder here against the map format"
	@echo "  make test            Run the validator's tests"
	@echo "  make fmt             Format and lint the code with ruff"
	@echo ""
	@echo "Example: make validate-maps"

validate-maps:
	uv run python validate_map.py $(MAP_FOLDERS)

test:
	uv run pytest

fmt:
	$(RUFF) format .
	$(RUFF) check --fix .
