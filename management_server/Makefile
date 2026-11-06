# `Makefile` runs the management server's tests, and formats and lints its code.
#
# Interface:
#     make help: list the targets.
#     make test: run the tests.
#     make fmt: format and lint with ruff.
#
# Example:
#     make test
RUFF ?= uv run ruff

.PHONY: help test fmt

help:
	@echo "Targets:"
	@echo "  make help   List the targets"
	@echo "  make test   Run the tests"
	@echo "  make fmt    Format and lint the code with ruff"
	@echo ""
	@echo "Example: make test"

test:
	uv run pytest

fmt:
	$(RUFF) format .
	$(RUFF) check --fix .
