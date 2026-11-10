# Makefile is the entry point of the pull-and-sprint tuning demo (W-202, pull toward wormholes and black holes in the
# game settings): `make help` lists the targets.
#
# Interface: make help | serve-demo [PORT=48510] | test | fmt | reset-artifacts [YES_TO_RESET_ARTIFACTS=1 | INTERACTIVE=1]
#
# Example: make serve-demo PORT=48510

PORT ?= 48510
RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (the settings and recordings the page saved). \
  Recreating them means tuning and playing again. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help serve-demo test fmt reset-artifacts

help:
	@echo "make serve-demo [PORT=48510]   Serve the tuning page on 127.0.0.1 and save what it sends to .artifacts/serve_demo/<run_id>/"
	@echo "make test                      Run the dynamics' tests (node --test)"
	@echo "make fmt                       Format and lint serve_demo.py (ruff)"
	@echo "make reset-artifacts           Delete .artifacts/ (needs YES_TO_RESET_ARTIFACTS=1 or INTERACTIVE=1)"
	@echo "Example: make serve-demo PORT=48510, then open http://127.0.0.1:48510/"

serve-demo:
	python3 serve_demo.py --port $(PORT)

test:
	node --test test_pull_dynamics.mjs

fmt:
	ruff format serve_demo.py
	ruff check serve_demo.py

reset-artifacts:
ifeq ($(YES_TO_RESET_ARTIFACTS),1)
	rm -rf .artifacts
else ifeq ($(INTERACTIVE),1)
	@printf '%s\nType yes to confirm: ' "$(RESET_ARTIFACTS_WARNING)"; read answer; [ "$$answer" = yes ]
	rm -rf .artifacts
else
	@printf '%s\nTo be asked instead, pass INTERACTIVE=1.\n' "$(RESET_ARTIFACTS_WARNING)" >&2; exit 2
endif
