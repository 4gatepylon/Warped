# `Makefile` serves the system prototype, runs its tests, and formats and lints its code.
#
# Interface:
#     make help: list the targets.
#     make run-prototype [PORT=<port>]: serve the prototype on this computer until Ctrl-C.
#     make test: run the tests.
#     make capture-flows: play every flow in Chrome, check each screen and screenshot it.
#     make fmt: format and lint with ruff.
#     make reset-artifacts YES_TO_RESET_ARTIFACTS=1 | INTERACTIVE=1: delete .artifacts/.
#
# Example:
#     make run-prototype
RUFF ?= uv run ruff
PORT ?= 8010

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (each run's database and command). \
  Recreating them means playing those runs again. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help run-prototype test capture-flows fmt reset-artifacts

help:
	@echo "Targets:"
	@echo "  make run-prototype     Serve the prototype at http://127.0.0.1:$(PORT)/ (PORT=$(PORT))"
	@echo "  make test              Run the tests"
	@echo "  make capture-flows     Play every flow in Chrome; screenshots in .artifacts/capture_flows/"
	@echo "  make fmt               Format and lint the code with ruff"
	@echo "  make reset-artifacts   Delete .artifacts/ (YES_TO_RESET_ARTIFACTS=1 or INTERACTIVE=1)"
	@echo ""
	@echo "Example: make run-prototype"

run-prototype:
	uv run system-prototype --port $(PORT)

test:
	uv run pytest

capture-flows:
	uv run python scripts/capture_flows.py

fmt:
	$(RUFF) format .
	$(RUFF) check --fix .

reset-artifacts:
ifeq ($(YES_TO_RESET_ARTIFACTS),1)
	rm -rf .artifacts
else ifeq ($(INTERACTIVE),1)
	@printf '%s\nType yes to confirm: ' "$(RESET_ARTIFACTS_WARNING)"; read answer; [ "$$answer" = yes ]
	rm -rf .artifacts
else
	@printf '%s\nTo be asked instead, pass INTERACTIVE=1.\n' "$(RESET_ARTIFACTS_WARNING)" >&2; exit 2
endif
