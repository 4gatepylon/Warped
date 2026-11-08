---
name: makefiles
description: Write or change a component's Makefile in this repo - which targets it must have and how to name them. Use whenever you add a Makefile, add, rename or document a Makefile target, or tell the user which make command to run.
---

# Makefiles

Each component folder (`game/`, `common/maps/`, each demo in `demos/`) has a
Makefile that is its entry point, whatever language the component is in: `make help` says how to
use it, and every other target says what it does and what it does it to, so `make <target>` reads
without opening the Makefile.

## Every Makefile

- `help` is the first target, so a plain `make` runs it. It lists every target with its arguments
  (e.g. `PORT=8000`) and a one-line description, and gives an example command.
- When a target is added, renamed or removed, `help` and the Makefile's `#` docstring change in the
  same commit.

## Required targets

- `make help`: lists the targets.
- `make test`: runs all the tests.
- `make fmt`: formats and lints the code with the language's tools (for Python, `ruff format` and
  `ruff check`).

Only when the component has them:

- `make clean`: removes the build outputs (`.build/`), and nothing else: never committed files or
  `.artifacts/`.
- `make reset-artifacts`: deletes the run outputs (`.artifacts/`). Like any destructive CLI
  (`clis` skill), it fails by default with a warning. `YES_TO_RESET_ARTIFACTS=1` deletes them
  without asking; `INTERACTIVE=1` asks, and deletes only if the user types `yes`:

```make
RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (logs, benchmark results, plots). \
  Recreating them means rerunning the scripts that wrote them. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

reset-artifacts:
ifeq ($(YES_TO_RESET_ARTIFACTS),1)
	rm -rf .artifacts
else ifeq ($(INTERACTIVE),1)
	@printf '%s\nType yes to confirm: ' "$(RESET_ARTIFACTS_WARNING)"; read answer; [ "$$answer" = yes ]
	rm -rf .artifacts
else
	@printf '%s\nTo be asked instead, pass INTERACTIVE=1.\n' "$(RESET_ARTIFACTS_WARNING)" >&2; exit 2
endif
```

## Target names

- Every other target that a person runs (every `.PHONY` target) is `<verb>-<noun>`:
  `run-management-server`, `serve-web-client`, `bench-physics`.
- A bare verb such as `make run`, `make serve`, `make build` or `make bench` is not allowed: it
  doesn't say what it runs, serves, builds or benchmarks.
- The only bare verbs allowed are `help`, `test`, `fmt` and `clean`, with the meanings above. A new
  one needs the user's agreement; add it to this skill in the same change.

## Must know

- Gotchas:
  - File targets such as `.build/client` are paths, not commands, so they keep path names.
