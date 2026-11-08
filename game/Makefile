# `Makefile` builds, tests, checks, formats and lints the Warped game (the workspace in this
# folder), and says what the machine it runs on can test (REFACTOR_PLAN.md, sections 7 and 8).
#
# Interface:
#     make help: list the targets.
#     make test: check formatting, lint with clippy, and run every crate's tests.
#     make fmt: format with rustfmt and lint with clippy.
#     make check-linux: check that warped-core and warped-server build for x86-64 Linux.
#     make check-environment: print the macOS version, chip, Metal device and toolchain versions.
#     make clean: remove .build/.
#     make reset-artifacts YES_TO_RESET_ARTIFACTS=1: delete .artifacts/.
#
# Example:
#     make test
LINUX_TARGET := x86_64-unknown-linux-musl

RESET_ARTIFACTS_WARNING := This deletes .artifacts/ (test logs, benchmark results, compared views). \
  Recreating them means rerunning the targets that wrote them. \
  To skip this question, pass YES_TO_RESET_ARTIFACTS=1.

.PHONY: help test fmt check-linux check-environment clean reset-artifacts

help:
	@echo "Targets:"
	@echo "  make test                                        Check formatting, lint, and run every crate's tests"
	@echo "  make fmt                                         Format with rustfmt and lint with clippy"
	@echo "  make check-linux                                 Check that warped-core and warped-server build for Linux"
	@echo "  make check-environment                           Print macOS, chip, Metal device and toolchain versions"
	@echo "  make clean                                       Remove .build/"
	@echo "  make reset-artifacts YES_TO_RESET_ARTIFACTS=1    Delete .artifacts/ (or INTERACTIVE=1 to be asked)"
	@echo ""
	@echo "Example: make test"

test:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --release -- -D warnings
	cargo test --workspace --release

fmt:
	cargo fmt --all
	cargo clippy --workspace --all-targets --release -- -D warnings

check-linux:
	rustup target add $(LINUX_TARGET)
	cargo check -p warped-core -p warped-server --release --target $(LINUX_TARGET)

check-environment:
	@sw_vers
	@echo "chip: $$(sysctl -n machdep.cpu.brand_string)"
	@echo "Metal:"; system_profiler SPDisplaysDataType 2>/dev/null | grep -E 'Chipset Model|Metal' || echo "  no Metal device found"
	@rustc --version; cargo --version
	@cmake --version 2>/dev/null | head -1 || echo "cmake: missing (brew install cmake)"
	@echo "Rust targets: $$(rustup target list --installed | tr '\n' ' ')"

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
