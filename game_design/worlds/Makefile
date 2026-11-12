# `Makefile` writes and checks the levels' world docs, runs the generator's tests, and formats and
# lints its code.
#
# Interface:
#     make help: list the targets.
#     make generate-world-docs: write every level's generated/ folder from its source/ folder.
#     make check-world-docs: fail if any level's generated/ folder is out of date.
#     make test: run the generator's tests.
#     make fmt: format and lint with ruff.
#
# Example:
#     make generate-world-docs
RUFF ?= uv run ruff
LEVEL_FOLDERS := $(patsubst %/source/world.yaml,%,$(wildcard */source/world.yaml))

.PHONY: help generate-world-docs check-world-docs test fmt

help:
	@echo "Targets:"
	@echo "  make help                  List the targets"
	@echo "  make generate-world-docs   Write every level's generated/ folder from its source/ folder"
	@echo "  make check-world-docs      Fail if any level's generated/ folder is out of date"
	@echo "  make test                  Run the generator's tests"
	@echo "  make fmt                   Format and lint the code with ruff"
	@echo ""
	@echo "Example: make generate-world-docs"

generate-world-docs:
	@test -n "$(LEVEL_FOLDERS)" || { echo "No level folder here has source/world.yaml" >&2; exit 2; }
	uv run python make_world_doc.py $(LEVEL_FOLDERS)

check-world-docs:
	@test -n "$(LEVEL_FOLDERS)" || { echo "No level folder here has source/world.yaml" >&2; exit 2; }
	uv run python make_world_doc.py --check $(LEVEL_FOLDERS)

test:
	uv run pytest

fmt:
	$(RUFF) format .
	$(RUFF) check --fix .
