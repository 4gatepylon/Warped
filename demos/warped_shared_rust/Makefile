# `Makefile` tests, formats and lints `warped_shared`, the library every Rust level demo shares, and
# tests the level crates that use it.
#
# Interface:
#     make help: list the targets.
#     make test: run the library's tests.
#     make test-levels: run `make test` in every level crate that uses the library (LEVELS=<folders>).
#     make fmt: format with rustfmt and lint with clippy.
#     make clean: remove .build/.
#
# Example:
#     make test-levels LEVELS="../hyperbolic_wormholes_rust ../dust_rust"
LEVELS ?= ../hyperbolic_wormholes_rust ../dust_rust ../snow_planet_rust ../city_sphere_rust ../warped_match_rust

.PHONY: help test test-levels fmt clean

help:
	@echo "Targets:"
	@echo "  make test                                        Run the library's tests"
	@echo "  make test-levels LEVELS=\"../dust_rust\"           Run make test in every level crate that uses the library"
	@echo "                                                   (default: Lunar Surface, Dust, Snow Planet, City Sphere, the match)"
	@echo "  make fmt                                         Format with rustfmt and lint with clippy"
	@echo "  make clean                                       Remove .build/"
	@echo ""
	@echo "Example: make test-levels LEVELS=\"../hyperbolic_wormholes_rust ../dust_rust\""

test:
	cargo test --release

test-levels:
	set -e; for level in $(LEVELS); do $(MAKE) -C $$level test; done

fmt:
	cargo fmt
	cargo clippy --release --all-targets -- -D warnings

clean:
	rm -rf .build
